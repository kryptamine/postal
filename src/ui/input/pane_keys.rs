use crate::{
    domain::folder::Role,
    domain::thread::Key,
    ui::exit::Exit,
    ui::input::keymap::Action,
    ui::input::step::{Drafting, Steer, Step},
    ui::list::inbox_view::InboxView,
    ui::popup::help::Help,
    ui::preview::link_hints::LinkHints,
    ui::preview::reading::Reading,
    ui::state::screen::Screen,
    ui::state::showing::Showing,
};

/// Acts on one key the panes take.
pub fn act(
    action: Option<Action>,
    view: &mut InboxView,
    screen: &mut Screen,
    half_page: usize,
) -> Step {
    match action {
        Some(Action::Quit) => Step::Exit(Exit::Quit),
        Some(action @ (Action::Read | Action::Step(_) | Action::Fold)) => {
            read(action, view, screen)
        }
        Some(Action::MarkUnread) => Step::Steer(Steer::MarkUnread),
        Some(Action::Delete) => Step::Steer(Steer::Delete),
        Some(Action::Archive) => Step::Steer(Steer::Archive),
        Some(Action::MoveTo) => Step::Steer(Steer::Pick),
        Some(Action::Undo) => Step::Steer(Steer::Undo),
        Some(Action::Compose) => Step::Draft(Drafting::Compose),
        Some(Action::Reply(to)) => Step::Reply(to),
        Some(Action::Forward) => Step::Forward,
        Some(Action::Back | Action::KeepQuery) => {
            view.close_menu();
            screen.list();
            Step::Stay
        }
        Some(Action::Open(to)) => Step::Open(to),
        Some(Action::Original) => Step::Original,
        Some(Action::SignIn) => screen
            .signed_out()
            .map_or(Step::Stay, |account| Step::Exit(Exit::SignIn(account))),
        Some(Action::Refresh) => Step::Steer(Steer::Refresh),
        Some(Action::Settings) => Step::Settings,
        Some(Action::Links) => Step::Links,
        Some(Action::Attachments(purpose)) => Step::Attachments(purpose),
        Some(Action::Folders) => Step::Steer(Steer::Folders),
        Some(action @ (Action::SwitchView | Action::SwitchQuoted)) => {
            reshow(action, view, screen);
            Step::Stay
        }
        Some(Action::Help) => {
            screen.help = Some(Help::default());
            Step::Stay
        }
        Some(Action::Search) => Step::Steer(Steer::Search),
        Some(Action::Type(c)) => Step::Steer(Steer::Type(c)),
        Some(Action::Erase) => Step::Steer(Steer::Erase),
        Some(Action::Complete(direction)) => Step::Steer(Steer::Complete(direction)),
        Some(Action::Accept) => Step::Steer(Steer::Accept),
        Some(Action::EndSearch) => {
            end_search(view, screen);
            Step::Stay
        }
        Some(action @ (Action::Move(_) | Action::Stretch(_))) => {
            walk(action, view);
            Step::Stay
        }
        Some(Action::Scroll(scroll)) => {
            match screen.showing.reading_mut() {
                Some(reading) => reading.scroll(scroll, half_page),
                None => view.scroll(scroll, half_page),
            }

            Step::Stay
        }
        None => Step::Stay,
    }
}

/// Moves the selection, dropping the Picks, or stretches them with it.
fn walk(action: Action, view: &mut InboxView) {
    match action {
        Action::Move(movement) => {
            view.picks.clear();
            view.apply(movement);
        }
        Action::Stretch(movement) => view.stretch(movement),
        _ => {}
    }
}

/// Closes the menu while the Query is typed, else ends the Search.
fn end_search(view: &mut InboxView, screen: &mut Screen) {
    let typing = screen.showing.mode().typing();
    if !typing && !view.picks.is_empty() {
        view.picks.clear();
        return;
    }

    if typing && view.close_menu() {
        screen.showing = Showing::Query;
        return;
    }

    view.end_search(typing);
    screen.list();
}

/// Shows HTML mail in the other View; on any other `action`, the Quote of
/// the selected Message, or in Reading of the one at the cursor.
fn reshow(action: Action, view: &mut InboxView, screen: &mut Screen) {
    if matches!(action, Action::SwitchView) {
        view.html = view.html.other();
        view.preview_scroll = 0;
        if let Some(reading) = screen.showing.reading_mut() {
            reading.reveal = true;
        }
    } else {
        let cursor = screen.showing.reading().map(|reading| reading.cursor);
        let message = cursor.map_or_else(
            || view.selected_message(),
            |cursor| view.stack().find(|m| m.body_key == cursor),
        );

        if let Some((account, body)) = message.map(|m| (m.account, m.body_key)) {
            view.switch_quoted(account, body);
        }
    }

    // The labels sit where the links were before the switch.
    screen.over.close::<LinkHints>();
}

/// Opens the selected Thread in Reading, or moves through it there.
fn read(action: Action, view: &mut InboxView, screen: &mut Screen) -> Step {
    match (action, screen.showing.reading_mut()) {
        (Action::Read, _) => {
            let Some(reading) = Reading::open(view.stack()) else {
                return Step::Stay;
            };

            screen.showing = Showing::Reading(reading);
            view.picks.clear();
            // Gmail keeps Trash and Spam out of All Mail, and their Threads
            // apart; a Search never shows theirs.
            let apart = view.search.is_none()
                && matches!(view.folder.role(), Some(Role::Trash | Role::Spam));
            let gmail = match view.selected_key() {
                Some(Key::Gmail(account, id)) if !apart => Some((account, id)),
                _ => None,
            };

            return Step::Steer(Steer::Read(gmail));
        }
        (Action::Step(direction), Some(reading)) => reading.step(direction, view.stack()),
        (Action::Fold, Some(reading)) => reading.fold(view.stack()),
        _ => {}
    }

    Step::Stay
}
