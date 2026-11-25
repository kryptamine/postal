use async_imap::{
    Session,
    imap_proto::{AttributeValue, Response},
};

use super::{
    Error, attributes,
    body_plan::{self, Plan, Sections},
    connect::Transport,
    tagged,
};
use crate::domain::uid::Uid;

/// Each of `uids` as `Plan` fetches it, `(uid, raw)`: a `BODYSTRUCTURE`
/// first, then one fetch per set of sections. What else the server says on
/// the way is dropped.
pub async fn fetch(
    session: &mut Session<Box<dyn Transport>>,
    uids: &str,
) -> Result<Vec<(Uid, Vec<u8>)>, Error> {
    let mut plans: Vec<(Uid, Plan)> = Vec::new();
    let command = format!("UID FETCH {uids} (UID BODYSTRUCTURE)");
    tagged::run(session, command, |response| {
        if let Response::Fetch(_, attributes) = response {
            plans.extend(plan(attributes));
        }
    })
    .await?;

    let mut groups: Vec<(String, Vec<Uid>)> = Vec::new();
    for (uid, plan) in &plans {
        let items = plan.items();
        match groups.iter_mut().find(|(i, _)| *i == items) {
            Some((_, uids)) => uids.push(*uid),
            None => groups.push((items, vec![*uid])),
        }
    }

    let mut bodies = Vec::new();
    for (items, uids) in groups {
        let set = uids
            .iter()
            .map(Uid::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut answered: Vec<(Uid, Sections)> = Vec::new();
        tagged::run(session, format!("UID FETCH {set} {items}"), |response| {
            if let Response::Fetch(_, attributes) = response {
                answered.extend(sections(attributes));
            }
        })
        .await?;

        for (uid, sections) in answered {
            if let Some((_, plan)) = plans.iter().find(|(u, _)| *u == uid) {
                bodies.push((uid, plan.assemble(&sections)));
            }
        }
    }

    Ok(bodies)
}

fn plan(attributes: &[AttributeValue<'_>]) -> Option<(Uid, Plan)> {
    let plan = Plan::of(attributes::structure(attributes)?);
    Some((attributes::uid(attributes)?, plan))
}

fn sections(attributes: &[AttributeValue<'_>]) -> Option<(Uid, Sections)> {
    let sections = attributes
        .iter()
        .filter_map(|a| match a {
            AttributeValue::BodySection {
                section,
                data: Some(data),
                ..
            } => Some((body_plan::key(section.as_ref())?, data.to_vec())),
            _ => None,
        })
        .collect();
    Some((attributes::uid(attributes)?, sections))
}
