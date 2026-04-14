use super::*;

pub(super) fn queue_post_render_intent<T>(behavior: &mut GraphshellTileBehavior<'_>, intent: T)
where
    T: Into<TilePendingIntent>,
{
    behavior.pending_post_render_intents.push(intent.into());
}

pub(super) fn extend_post_render_intents<I, T>(
    behavior: &mut GraphshellTileBehavior<'_>,
    intents: I,
) where
    I: IntoIterator<Item = T>,
    T: Into<TilePendingIntent>,
{
    behavior
        .pending_post_render_intents
        .extend(intents.into_iter().map(Into::into));
}

