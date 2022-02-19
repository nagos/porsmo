use crate::{
    input::{listen_for_inputs, Command},
    terminal::TermRawMode,
};
use anyhow::Result;
use porsmo::{
    counter::Counter,
    pomodoro::{Mode, Pomodoro},
    stopwatch::Stopwatch,
};
use std::{io::Write, sync::mpsc::Receiver, thread, time::Duration};

pub fn pomodoro(work_time: u64, break_time: u64, long_break_time: u64) -> Result<u64> {
    use ui::*;
    let mut pomo = Pomodoro::new(work_time, break_time, long_break_time);
    let mut skip_prompt = false;
    let stdout = &mut TermRawMode::new().stdout;
    let rx = listen_for_inputs();

    loop {
        match rx.try_recv() {
            Ok(Command::Quit) => {
                break;
            }

            Ok(Command::Pause) => {
                pomo.pause();
            }

            Ok(Command::Resume) => {
                pomo.resume();
            }

            Ok(Command::Toggle) => {
                pomo.toggle();
            }

            Ok(Command::Enter) => {
                if skip_prompt {
                    pomo.next_mode();
                    skip_prompt = false;
                } else {
                    pomo.toggle();
                }
            }

            Ok(Command::Yes) => {
                if skip_prompt {
                    pomo.next_mode();
                    skip_prompt = false;
                }
            }

            Ok(Command::Skip) => {
                pomo.pause();
                skip_prompt = true;
            }

            Ok(Command::No) => {
                skip_prompt = false;
                pomo.resume();
            }

            _ => (),
        }

        if skip_prompt {
            show_prompt(stdout, pomo.mode())?;
        } else {
            if pomo.has_ended() {
                alert(pomo.check_next_mode());
                let (counter, next) =
                    start_excess_counting(&rx, stdout, pomo.check_next_mode(), pomo.session())?;
                if next {
                    pomo.next_mode();
                } else {
                    return Ok(counter);
                }
            }

            show_counter(stdout, pomo.counter(), pomo.is_running())?;
        }

        show_session(stdout, pomo.session())?;

        thread::sleep(Duration::from_millis(100));
    }

    Ok(pomo.counter())
}

fn start_excess_counting(
    rx: &Receiver<Command>,
    stdout: &mut impl Write,
    next_mode: Mode,
    session: u64,
) -> Result<(u64, bool)> {
    use ui::*;
    let mut st = Stopwatch::new(0);

    loop {
        match rx.try_recv() {
            Ok(Command::Quit) => {
                st.end_count();
                break;
            }

            Ok(Command::Pause) => {
                st.pause();
            }

            Ok(Command::Resume) => {
                st.resume();
            }

            Ok(Command::Toggle) => {
                st.toggle();
            }

            Ok(Command::Enter) => return Ok((st.counter(), true)),

            _ => (),
        }

        show_mode_change(stdout, next_mode, session, st.counter(), st.is_running())?;
        show_session(stdout, session)?;

        thread::sleep(Duration::from_millis(100));
    }

    Ok((st.counter(), false))
}

// Purely UI and User related
mod ui {
    use crate::{
        format::fmt_time,
        notification::notify_default,
        pomodoro::Mode,
        sound::play_bell,
        terminal::{
            clear, show_message, show_message_green, show_message_red, show_message_yellow,
            show_time_paused, show_time_running,
        },
    };
    use anyhow::Result;
    use std::{io::Write, thread};

    pub fn show_counter(stdout: &mut impl Write, counter: u64, running: bool) -> Result<()> {
        if running {
            show_time_running(stdout, counter)?;
            show_message(stdout, "[Q]: quit, [Space]: pause/resume", 2)?;
        } else {
            show_time_paused(stdout, counter)?;
            show_message(stdout, "[Q]: quit, [Space]: pause/resume", 2)?;
        }

        Ok(())
    }

    pub fn alert(next_mode: Mode) {
        let heading;
        let message;

        match next_mode {
            Mode::Work => {
                heading = "You break ended!";
                message = "Time for some work"
            }
            Mode::Break => {
                heading = "Pomodoro ended!";
                message = "Time for a short break"
            }
            Mode::LongBreak => {
                heading = "Pomodoro 4 sessions complete!";
                message = "Time for a long break"
            }
        }

        thread::spawn(move || {
            notify_default(heading, message).unwrap();
            play_bell().unwrap();
        });
    }

    pub fn show_prompt(stdout: &mut impl Write, mode: Mode) -> Result<()> {
        clear(stdout)?;
        match mode {
            Mode::Work => show_message(stdout, "skip this work session?", 0)?,
            Mode::Break => show_message(stdout, "skip this break?", 0)?,
            Mode::LongBreak => show_message(stdout, "skip this long break?", 0)?,
        };

        show_message(stdout, "[Q]: Quit, [Enter]: Yes, [N]: No", 2)?;

        Ok(())
    }

    pub fn show_session(stdout: &mut impl Write, session: u64) -> Result<()> {
        show_message_yellow(stdout, &format!("(Round: {})", session), 1)
    }

    pub fn show_extended_time(stdout: &mut impl Write, counter: u64, running: bool) -> Result<()> {
        if running {
            show_message_green(stdout, &format!("-{}", fmt_time(counter)), 3)?;
        } else {
            show_message_red(stdout, &format!("-{}", fmt_time(counter)), 3)?;
        }

        Ok(())
    }

    pub fn show_mode_change(
        stdout: &mut impl Write,
        next_mode: Mode,
        session: u64,
        counter: u64,
        running: bool,
    ) -> Result<()> {
        clear(stdout)?;
        match next_mode {
            Mode::Work => show_message_red(stdout, "start work?", 0)?,
            Mode::Break => show_message_green(stdout, "start break?", 0)?,
            Mode::LongBreak => show_message_green(stdout, "start long break?", 0)?,
        }

        show_session(stdout, session)?;

        show_message(
            stdout,
            "[Q]: Quit, [Enter]: Start, [Space]: Toggle excess counter",
            2,
        )?;

        show_extended_time(stdout, counter, running)?;

        Ok(())
    }
}
