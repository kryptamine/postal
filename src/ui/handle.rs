use std::io;

use crossterm::event::{Event, KeyEvent};

use crate::{
    domain::folder::Role,
    domain::shown::Shown,
    domain::thread::Key,
    ui::composing,
    ui::draft_preview::DraftPreview,
    ui::exit::Exit,
    ui::inbox_view::InboxView,
    ui::keymap::{self, Action, DraftKey, LocalKey, Pick, QueuedKey},
    ui::link_hints::Typed,
    ui::main_screen::Ui,
    ui::over::Layer,
    ui::reading::Reading,
    ui::run::Error,
    ui::screen::Screen,
    ui::search::Search,
    ui::settings,
    ui::showing::Showing,
    ui::step::{Drafting, Steer, Step},
    ui::toast::Toast,
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
        Some(Ok(Event::Key(key))) => Ok(match screen.over.top() {
            Some(Layer::Settings(_)) => on_settings(key, screen),
            Some(Layer::Draft(_)) => on_draft(key, screen),
            Some(Layer::LocalDrafts(_)) => on_local(key, screen),
            Some(Layer::Picker(_)) => pick(key, screen),
            Some(Layer::Hints) => on_hints(key, screen),
            None => act(
                keymap::action(key, screen.showing.mode()),
                view,
                screen,
                half_page,
            ),
        }),
        // The labels sit where the old width laid the links out.
        Some(Ok(Event::Resize(..))) => {
            screen.over.hints = None;
            Ok(Step::Stay)
        }
        Some(Ok(_)) => Ok(Step::Stay),
    }
}

fn on_hints(key: KeyEvent, screen: &mut Screen) -> Step {
    let typed = screen
        .over
        .hints
        .as_mut()
        .map_or(Typed::Cancel, |h| h.key(key));
    if !matches!(typed, Typed::Partial) {
        screen.over.hints = None;
    }

    match typed {
        Typed::Open(url) => Step::Browse(url),
        Typed::Partial | Typed::Cancel => Step::Stay,
    }
}

fn act(
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
        Some(Action::Reply { all }) => Step::Reply(all),
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
    screen.over.hints = None;
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

/// Acts on one key in Settings, which takes them all.
fn on_settings(key: KeyEvent, screen: &mut Screen) -> Step {
    let Some(settings) = &mut screen.over.settings else {
        return Step::Stay;
    };

    match settings.key(key) {
        settings::Step::Stay => Step::Stay,
        settings::Step::Close { reload } => {
            screen.over.settings = None;
            if reload {
                Step::Exit(Exit::Reload)
            } else {
                Step::Stay
            }
        }
        settings::Step::Help => {
            screen.help = true;
            Step::Stay
        }
        settings::Step::Icons(choice) => Step::Icons(choice),
        settings::Step::AddAccount => Step::Exit(Exit::AddAccount),
        settings::Step::Unlink(email) => Step::Exit(Exit::Unlink(email)),
        settings::Step::Quit => Step::Exit(Exit::Quit),
    }
}

/// Acts on one key in a Draft's preview, or an Outbox Message's.
fn on_draft(key: KeyEvent, screen: &mut Screen) -> Step {
    if screen
        .over
        .draft
        .as_ref()
        .is_some_and(|preview| preview.queued)
    {
        match keymap::queued(key) {
            Some(QueuedKey::Close) => screen.over.draft = None,
            Some(QueuedKey::Help) => screen.help = true,
            Some(QueuedKey::Quit) => return Step::Exit(Exit::Quit),
            None => {}
        }

        return Step::Stay;
    }

    match keymap::draft(key) {
        Some(DraftKey::Quit) => Step::Exit(Exit::Quit),
        Some(DraftKey::Help) => {
            screen.help = true;
            Step::Stay
        }
        Some(DraftKey::Send) => Step::Draft(Drafting::Send),
        Some(DraftKey::Edit) => Step::Draft(Drafting::Edit),
        Some(DraftKey::Keep) => {
            composing::keep(screen);
            Step::Stay
        }
        Some(DraftKey::Discard) => {
            composing::discard(screen);
            Step::Stay
        }
        None => Step::Stay,
    }
}

fn on_local(key: KeyEvent, screen: &mut Screen) -> Step {
    let Some(local) = screen.over.local.as_mut() else {
        return Step::Stay;
    };

    match keymap::local(key) {
        Some(LocalKey::Down) => local.down(),
        Some(LocalKey::Up) => local.up(),
        Some(LocalKey::Open) => {
            let Some(row) = local.chosen() else {
                return Step::Stay;
            };

            let path = row.path.clone();
            let read = if row.queued {
                DraftPreview::queued(path)
            } else {
                DraftPreview::read(path, None)
            };

            match read {
                Ok(preview) => screen.over.draft = Some(preview),
                Err(error) => {
                    tracing::error!("reading the Draft failed: {error}");
                    screen.toast = Some(Toast::error("could not read the Draft"));
                }
            }
        }
        Some(LocalKey::Discard) => {
            let draft = local.chosen().filter(|row| !row.queued);
            if let Some(path) = draft.map(|row| row.path.clone()) {
                composing::discard_file(screen, path);
            }
        }
        Some(LocalKey::Undo) => return Step::Steer(Steer::Undo),
        Some(LocalKey::Close) => screen.over.local = None,
        Some(LocalKey::Help) => screen.help = true,
        Some(LocalKey::Quit) => return Step::Exit(Exit::Quit),
        None => {}
    }

    Step::Stay
}

/// Acts on one key in the Folder Picker, which takes them all.
fn pick(key: KeyEvent, screen: &mut Screen) -> Step {
    let Some(picker) = screen.over.picker.as_mut() else {
        return Step::Stay;
    };

    match keymap::pick(key) {
        Some(Pick::Type(c)) => picker.push(c),
        Some(Pick::Erase) => picker.pop(),
        Some(Pick::Down) => picker.down(),
        Some(Pick::Up) => picker.up(),
        Some(Pick::Choose) => {
            let chosen = picker.chosen().cloned();
            if let Some(folder) = chosen {
                screen.over.picker = None;
                if matches!(folder, Shown::LocalDrafts) {
                    return Step::LocalDrafts;
                }

                return Step::Steer(Steer::Show(folder));
            }
        }
        Some(Pick::Close) => screen.over.picker = None,
        Some(Pick::Quit) => return Step::Exit(Exit::Quit),
        None => {}
    }

    Step::Stay
}
