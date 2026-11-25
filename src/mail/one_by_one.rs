use crate::{domain::uid::Uid, mail::imap, mail::sync::Error};

/// `fetch` of `uids`; when the server refuses the batch, each is asked again
/// alone, so one Message it cannot serve does not take the rest with it.
pub async fn fetch<T>(
    uids: &[Uid],
    what: &str,
    mut fetch: impl AsyncFnMut(&[Uid]) -> Result<Vec<T>, imap::Error>,
) -> Result<Vec<T>, Error> {
    match fetch(uids).await {
        Err(error) if error.refusal().is_some() && uids.len() > 1 => {
            let mut fetched = Vec::new();
            for &uid in uids {
                match fetch(&[uid]).await {
                    Ok(one) => fetched.extend(one),
                    Err(error) if error.refusal().is_some() => {
                        tracing::warn!("fetching the {what} of {uid} was refused: {error}");
                    }
                    Err(error) => return Err(error.into()),
                }
            }

            Ok(fetched)
        }
        Err(error) if error.refusal().is_some() => {
            tracing::warn!("fetching the {what} of {uids:?} was refused: {error}");
            Ok(Vec::new())
        }
        result => Ok(result?),
    }
}
