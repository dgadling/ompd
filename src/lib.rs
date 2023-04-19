mod back_filler;
use back_filler::BackFiller;
mod capturer;
pub mod config;
mod dir_manager;
pub mod movie_maker;

use capturer::Capturer;
use chrono::Local;
use config::Config;
use dir_manager::DirManager;
use log::{error, info, warn};
use movie_maker::MovieMaker;
use std::thread;

pub fn run(config: Config) {
    let sleep_interval = std::time::Duration::from_secs(config.interval);
    let mut d = DirManager::new(&config.shot_output_dir, &config.vid_output_dir);
    let mut c = Capturer::new(&sleep_interval);

    let starting_time = Local::now();
    let mut last_time = starting_time;

    if config.handle_old_dirs_on_startup {
        let config_to_move = config.clone();
        let starting_time_to_move = starting_time;

        let backfiller_maybe = thread::Builder::new()
            .name("backfill".into())
            .spawn(move || {
                info!("Going back and re-making movies!");

                let b = BackFiller::new(config_to_move, starting_time_to_move);
                b.run();
            });

        if let Err(e) = backfiller_maybe {
            warn!("Couldn't spawn backfill thread! {e:?}");
        }
    }

    let made_output_d = d.make_shot_output_dir();
    if let Err(e) = made_output_d {
        error!("Couldn't make an output directory: {e:?}");
        panic!("Couldn't make an output directory!");
    }

    c.discover_current_frame(&mut d);

    loop {
        let capture_result = c.capture_screen();
        if let Err(e) = capture_result {
            info!("Couldn't get a good screenshot ({:?}), skip this frame", e);
            thread::sleep(sleep_interval);
            continue;
        }

        let now = Local::now();

        // NOTE: Timezone changes are handled correctly in subtraction, so this can only go
        // backwards if the timezone doesn't change but the system clock goes backwards.
        if (now - last_time).num_seconds() > config.max_sleep_secs {
            // At this point we know we went *forward* in time since max_sleep_secs can only be
            // positive.
            let change_result = c.deal_with_change(&d, &last_time, &now);
            match change_result {
                Err(e) => {
                    error!("Some issue dealing with a decent time gap: {e:?}");
                    info!("Going to sleep and try again");
                    thread::sleep(sleep_interval);
                    continue;
                }
                Ok(capturer::ChangeType::NewDay) => {
                    info!("Brand new day! Let's goooooo");

                    let shot_dir = d.get_current_shot_dir();
                    let config_to_move = config.clone();
                    let moviemaker_maybe =
                        thread::Builder::new()
                            .name("moviemaker".into())
                            .spawn(move || {
                                // TODO: Fire up a resizer before doing the movie making, compress when done.
                                info!("Launching movie maker");
                                let m = MovieMaker::new(config_to_move);
                                m.make_movie_from(shot_dir.as_path());
                            });

                    if let Err(e) = moviemaker_maybe {
                        warn!("Couldn't spawn movie maker thread! {e:?}");
                    }

                    // Get ready for today to make sure we have the right path to make movies in.
                    let made_output_dir = d.make_shot_output_dir();
                    if let Err(e) = made_output_dir {
                        error!("Couldn't make new output directory?!: {e:?}");
                        break;
                    }
                    c.set_current_frame(0);
                }
                Ok(capturer::ChangeType::Nop) => {}
            }
        }

        c.store(capture_result.unwrap(), d.current_shot_dir());
        last_time = now;

        thread::sleep(sleep_interval);
    }
}
