extern crate rusqlite;
extern crate clap;
extern crate rayon;

use std::{thread, time};
use std::process::Command;

use clap::{Arg, App};
use rayon::prelude::*;

mod notificationcenter;
mod configuration;

fn show_matching_applications(app_name: &str, conn: &rusqlite::Connection) {
    let results = notificationcenter::lookup_app_id(app_name, conn);
    for app_id_result in results {
        if let Ok(app_result) = app_id_result {
                println!("Matched application: {} -- app_id: {}",
                app_result.bundleid,
                app_result.app_id)
        }
    }
}

fn main() {
    let matches = App::new("osxnotifysounds")
                          .version("1.0")
                          .author("Joseph Gimenez <joseph.gimenez@snagajob.com>")
                          .about("Define custom sounds for Notification Center Alerts")
                          .arg(Arg::with_name("APP_NAME")
                               .short("a")
                               .help("Lookup app_id for application")
                               .takes_value(true))
                          .get_matches();

    let config_json = configuration::load();
    let conn = notificationcenter::open_notificationcenter_db();

    if let Some(app_name) = matches.value_of("APP_NAME") {
        show_matching_applications(app_name, &conn);
        std::process::exit(0);
    }

    let mut app_notes = notificationcenter::populate_app_notes(&config_json, &conn);

    loop {
        for app_entry in &mut app_notes {
            let app_id = app_entry.details.get("app_id").unwrap().as_u64().unwrap() as u32;
            let latest_alerts =
                notificationcenter::get_newest_alerts_for_app(app_entry.note_id,app_id, &conn);

            for alert in latest_alerts {
                match alert {
                    Ok(alert_data) => {
                        let encoded_data = alert_data.encoded_data;
                        let encoded_data = String::from_utf8_lossy(&encoded_data);
                        let note_iter : Vec<_> =
                            app_entry.details["notification_details"]
                                     .as_object().unwrap().iter().collect();

                        note_iter.par_iter().for_each(|&notification_details| {
                            let notification_details = notification_details.1;
                            let look_for = &notification_details["look_for"];
                            let sound = &notification_details["sound"];

                            // iterate through each look_for item and see if any are found in alert text
                            if look_for
                                .as_array()
                                .expect("'look_for' json is not an array")
                                .iter()
                                .any(|data| encoded_data.contains(data.as_str().unwrap())) {
                                    Command::new("sh")
                                    .arg("-c")
                                    .arg(&format!("afplay {}", sound))
                                    .output()
                                    .expect("afplay failed??");
                                }
                        });
                        // update our latest note counter, so don't play custom sounds on old alerts
                        app_entry.note_id = alert_data.note_id;
                    }
                    Err(_) => continue,
                };
            }
            thread::sleep(time::Duration::from_secs(1));
        }
    }
}
