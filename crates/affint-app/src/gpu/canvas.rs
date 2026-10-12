//! Private candidate publication and last-submission ownership for the native host.
use affint_document::Revision;
use affint_render::{OutputFormat, RenderRequest};

use crate::{
    editor::AppResult,
    upload::{Canvas, Context, Key, Retirement},
};

pub(super) struct CanvasSet {
    context: Context,
    retirement: Retirement,
    current: Option<Canvas>,
    candidate: Option<Canvas>,
}
impl CanvasSet {
    pub(super) fn new(context: Context, retirement: Retirement) -> Self {
        Self {
            context,
            retirement,
            current: None,
            candidate: None,
        }
    }
    pub(super) fn context(&self) -> &Context {
        &self.context
    }
    pub(super) fn accepted_key(&self) -> Option<Key> {
        self.current.as_ref().map(|canvas| canvas.key)
    }
    pub(super) fn candidate(&self) -> Option<&Canvas> {
        self.candidate.as_ref()
    }
    pub(super) fn sample(&self) -> Option<&Canvas> {
        self.candidate.as_ref().or(self.current.as_ref())
    }
    pub(super) fn offer(
        &mut self,
        canvas: Canvas,
        desired: Option<(Revision, RenderRequest, u64, u64)>,
    ) -> AppResult<()> {
        if !canvas.key.matches(desired)
            || canvas.key.generation != self.context.generation
            || canvas.key.request.format() != OutputFormat::LinearSrgbRgbaF32Premultiplied
        {
            return Err("Completed canvas no longer matches this view/device/format.".into());
        }
        if self.candidate.is_some() {
            return Err("A private canvas is still awaiting presentation.".into());
        }
        self.candidate = Some(canvas);
        Ok(())
    }
    pub(super) fn keep_desired(&mut self, desired: Option<(Revision, RenderRequest, u64, u64)>) {
        if self
            .candidate
            .as_ref()
            .is_some_and(|canvas| !canvas.key.matches(desired))
        {
            self.discard_candidate();
        }
    }
    pub(super) fn discard_candidate(&mut self) {
        if let Some(canvas) = self.candidate.take() {
            self.retirement.retire(&self.context, canvas);
        }
    }
    /// Called only after validated display conversion and the final Hybrid submission.
    pub(super) fn promote_after_submit(
        &mut self,
        desired: Option<(Revision, RenderRequest, u64, u64)>,
        sampled: bool,
    ) -> AppResult<bool> {
        if !sampled || self.candidate.is_none() {
            return Ok(false);
        }
        let key = self
            .candidate
            .as_ref()
            .ok_or("Private canvas disappeared before publication.")?
            .key;
        if !key.matches(desired) || key.generation != self.context.generation {
            self.discard_candidate();
            return Err(
                "Canvas became obsolete before publication; the accepted image is retained.".into(),
            );
        }
        let canvas = self
            .candidate
            .take()
            .ok_or("Private canvas disappeared before publication.")?;
        if let Some(retired) = self.current.replace(canvas) {
            self.retirement.retire(&self.context, retired);
        }
        Ok(true)
    }
    pub(super) fn clear(&mut self) {
        self.discard_candidate();
        if let Some(canvas) = self.current.take() {
            self.retirement.retire(&self.context, canvas);
        }
    }
}
impl Drop for CanvasSet {
    fn drop(&mut self) {
        self.clear();
    }
}
