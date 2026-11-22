use crate::{
    domain::folder::Role,
    domain::thread::Key,
    ui::exit::Exit,
    ui::keymap::Action,
    ui::list::inbox_view::InboxView,
    ui::list::search::Search,
    ui::preview::link_hints::LinkHints,
    ui::preview::reading::Reading,
    ui::screen::Screen,
    ui::showing::Showing,
    ui::step::{Drafting, Steer, Step},
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
        Some(Action::Undo) => Step::Steer(Steer::Undo),
        Some(Action::Compose) => Step::Draft(Drafting::Compose),
        Some(Action::Reply(to)) => Step::Reply(to),
        Some(Action::Back | Action::KeepQuery) => {
            screen.list();
            Step::Stay
        }
        Some(Action::Open(to)) => Step::Open(to),
        Some(Action::SignIn) => screen
            .signed_out()
            .map_or(Step::Stay, |account| Step::Exit(Exit::SignIn(account))),
        Some(Action::Settings) => Step::Settings,
        Some(Action::Links) => Step::Links,
        Some(Action::Folders) => Step::Steer(Steer::Folders),
        Some(action @ (Action::SwitchView | Action::SwitchQuoted)) => {
            reshow(action, view, screen);
            Step::Stay
        }
        Some(Action::Help) => {
            screen.help = true;
            Step::Stay
        }
        Some(Action::Search) => Step::Steer(Steer::Search),
        Some(Action::Type(c)) => {
            view.edit_query(|search| search.push(c));
            Step::Stay
        }
        Some(Action::Erase) => {
            view.edit_query(Search::pop);
            Step::Stay
        }
        Some(Action::EndSearch { restore }) => {
            view.end_search(restore);
            screen.list();
            Step::Stay
        }
        Some(Action::Move(movement)) => {
            view.apply(movement);
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
fn read(action: Action, view: &InboxView, screen: &mut Screen) -> Step {
    match (action, screen.showing.reading_mut()) {
        (Action::Read, _) => {
            let Some(reading) = Reading::open(view.stack()) else {
                return Step::Stay;
            };

            screen.showing = Showing::Reading(reading);
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
        (Action::Fold, Some(reading)) => reading.fold(),
        _ => {}
    }

    Step::Stay
}
