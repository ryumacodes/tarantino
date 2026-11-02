use super::*;
use crate::video_processing::{shortcut_overlay::Prepared,shortcut_stage::ShortcutStage};
impl GpuCompositor {
    pub(crate) fn configure_captions(&mut self,prepared:Option<crate::video_processing::caption_overlay::Prepared>)->Result<()> {
        self.caption_stage=prepared.map(|prepared|crate::video_processing::caption_stage::CaptionStage::new(
            &self.device,&self.queue,prepared,self.output_texture.width(),self.output_texture.height())).transpose()?;
        Ok(())
    }
    pub(crate) fn set_caption_frame(&mut self,frame:u64) {self.caption_frame=frame;}

    pub(crate) fn configure_shortcuts(&mut self,prepared:Option<&Prepared>)->Result<bool> {
        self.shortcut_draws.clear();
        let Some(prepared)=prepared.filter(|p|p.animated()) else {self.shortcut_stage=None;return Ok(false)};
        self.shortcut_stage=Some(ShortcutStage::new(&self.device,&self.queue,prepared.rasters(),
            self.output_texture.width(),self.output_texture.height())?);
        Ok(true)
    }
    pub(crate) fn set_shortcut_frame(&mut self,prepared:&Prepared,frame:u64) {
        self.shortcut_draws.clear();self.shortcut_draws.extend_from_slice(prepared.draws(frame));
    }
}
