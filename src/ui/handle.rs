use std::io;

use crossterm::event::{Event, KeyEvent};

use crate::{
    ui::exit::Exit, ui::keymap, ui::main_screen::Ui, ui::pane_keys, ui::popup::layer_act::Act,
    ui::run::Error, ui::screen::Screen, ui::step::Step,
};

/// Acts on one input event, where the view alone can take it.
pub fn handle(
    event: Option<io::Result<Event>>,
    ui: &mut Ui,
    half_page: usize,
) -> Result<Step, Error> {
    let Ui { view, screen } = ui;
    match event {
        None => Ok(Step::Exit(Exit::Quit)),
        Some(Err(source)) => Err(Error::Terminal(source)),
        Some(Ok(Event::Key(key))) if screen.help => {
            screen.help = !keymap::closes_help(key);
            Ok(Step::Stay)
        }
        Some(Ok(Event::Key(key))) if !keymap::pressed(key) => Ok(Step::Stay),
        Some(Ok(Event::Key(key))) if screen.over.top().is_some() => Ok(on_top(key, screen)),
        Some(Ok(Event::Key(key))) => Ok(pane_keys::act(
            keymap::action(key, screen.showing.mode()),
            view,
            screen,
            half_page,
        )),
        Some(Ok(Event::Resize(..))) => {
            screen.over.resized();
            Ok(Step::Stay)
        }
        Some(Ok(_)) => Ok(Step::Stay),
    }
}

/// Acts on one key in the layer taking them.
fn on_top(key: KeyEvent, screen: &mut Screen) -> Step {
    let Some(layer) = screen.over.top_mut() else {
        return Step::Stay;
    };

    match layer.key(key) {
        Act::Stay => Step::Stay,
        Act::Help => {
            screen.help = true;
            Step::Stay
        }
        Act::Close(step) => {
            screen.over.close_top();
            step
        }
        Act::Step(step) => step,
        Act::Open(layer) => {
            screen.over.open(*layer);
            Step::Stay
        }
        Act::Toast(toast) => {
            screen.toast = Some(toast);
            Step::Stay
        }
    }
}
