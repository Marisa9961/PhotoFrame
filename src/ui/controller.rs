use super::*;

impl Studio {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let loaded = Settings::load();
        loaded
            .as_ref()
            .map(|settings| settings.language)
            .unwrap_or_default()
            .activate();
        let (settings, status) = match loaded {
            Ok(s) => (s, tr!("status.ready")),
            Err(e) => (Settings::default(), tr!("errors.config", error = e)),
        };
        let color_input = cx.new(|cx| TextInput::new(hex(settings.style.color), cx));
        let title_input = cx.new(|cx| TextInput::new(&settings.style.title, cx));
        let subtitle_input = cx.new(|cx| TextInput::new(&settings.style.subtitle, cx));
        let template_name = cx.new(|cx| TextInput::new(tr!("text.templates"), cx));
        let title_subscription = cx.subscribe(&title_input, |this, input, _: &Changed, cx| {
            this.settings.style.title = input.read(cx).value();
            this.refresh(PreviewTrigger::Input, cx);
        });
        let subtitle_subscription =
            cx.subscribe(&subtitle_input, |this, input, _: &Changed, cx| {
                this.settings.style.subtitle = input.read(cx).value();
                this.refresh(PreviewTrigger::Input, cx);
            });
        let subscription = cx.subscribe(&color_input, |this, input, _: &Changed, cx| {
            if let Some(color) = parse_color(&input.read(cx).value()) {
                this.settings.style.color = color;
                this.refresh(PreviewTrigger::Input, cx);
            } else {
                this.status = tr!("errors.color");
                cx.notify();
            }
        });
        let release_subscription = cx.on_release(|this, _| {
            this.thumbnails.cancel_active();
            if let Err(e) = this.flush_preferences() {
                eprintln!("{}", tr!("errors.save_exit", error = e));
            }
        });
        let quit_subscription = cx.on_app_quit(|this, _| {
            if let Err(e) = this.flush_preferences() {
                eprintln!("{}", tr!("errors.save_exit", error = e));
            }
            async {}
        });
        Self {
            persisted_settings: settings.clone(),
            preference_error: None,
            settings,
            photos: Arc::default(),
            selected: 0,
            preview: preview_coordinator::PreviewCoordinator::default(),
            exports: export_session::ExportSession::default(),
            thumbnails: thumbnails::Thumbnails::default(),
            thumbnail_task: None,
            filmstrip_scroll: ScrollHandle::new(),
            filmstrip_width: 0.,
            styles_open: true,
            composition_open: false,
            layout_open: false,
            text_open: false,
            export_dialog: None,
            settings_dialog: None,
            pending_preset_restore: None,
            status,
            color_input,
            title_input,
            subtitle_input,
            template_name,
            _subscriptions: vec![
                subscription,
                title_subscription,
                subtitle_subscription,
                release_subscription,
                quit_subscription,
            ],
        }
    }
    pub(super) fn flush_preferences(&mut self) -> anyhow::Result<()> {
        if self.settings != self.persisted_settings {
            if let Err(e) = self.settings.save() {
                self.preference_error = Some(tr!("errors.save", error = e));
                return Err(e);
            }
            self.persisted_settings = self.settings.clone();
        }
        self.preference_error = None;
        Ok(())
    }
    pub(super) fn sync_inputs(&self, cx: &mut Context<Self>) {
        self.color_input.update(cx, |input, cx| {
            input.set(&hex(self.settings.style.color), cx)
        });
        self.title_input
            .update(cx, |input, cx| input.set(&self.settings.style.title, cx));
        self.subtitle_input
            .update(cx, |input, cx| input.set(&self.settings.style.subtitle, cx));
    }
    pub fn add(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.export_dialog.is_some() || self.settings_dialog.is_some() {
            return;
        }
        let before = self.photos.len();
        let mut rejected = 0;
        for path in paths {
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if path.is_file()
                && ["jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp"].contains(&ext.as_str())
            {
                let path = path.canonicalize().unwrap_or(path);
                if !self.photos.contains(&path) {
                    Arc::make_mut(&mut self.photos).push(path);
                }
            } else {
                rejected += 1;
            }
        }
        self.status = tr!(
            "status.added",
            count = self.photos.len() - before,
            skipped = if rejected > 0 {
                tr!("status.skipped", count = rejected)
            } else {
                String::new()
            }
        );
        if before == 0 && !self.photos.is_empty() {
            self.selected = 0;
            self.refresh(PreviewTrigger::Immediate, cx);
        }
        cx.notify();
    }
    pub(super) fn choose(&mut self, kind: FileChoice, cx: &mut Context<Self>) {
        let response = cx.prompt_for_paths(PathPromptOptions {
            files: kind != FileChoice::Output,
            directories: kind == FileChoice::Output,
            multiple: kind == FileChoice::Photos,
            prompt: Some(
                match kind {
                    FileChoice::Photos => tr!("prompts.photos"),
                    FileChoice::Font => tr!("prompts.font"),
                    FileChoice::Output => tr!("prompts.output"),
                }
                .into(),
            ),
        });
        cx.spawn(async move |this, cx| match response.await {
            Ok(Ok(Some(paths))) => {
                let _ = this.update(cx, |this, cx| {
                    if kind == FileChoice::Photos {
                        this.add(paths, cx);
                        return;
                    }
                    if let Some(path) = paths.into_iter().next() {
                        match kind {
                            FileChoice::Font => this.settings.style.font = Some(path),
                            FileChoice::Output => this.settings.output = Some(path),
                            FileChoice::Photos => unreachable!("Photo imports are handled above"),
                        }
                        if kind == FileChoice::Output {
                            if let Some(all) = this.exports.pending_scope.take() {
                                this.export(all, cx);
                            }
                            cx.notify();
                            return;
                        }
                        this.refresh(PreviewTrigger::Immediate, cx);
                    }
                });
            }
            Ok(Err(e)) => {
                let _ = this.update(cx, |this, cx| {
                    this.status = tr!("errors.choose_file", error = e);
                    if kind == FileChoice::Output {
                        this.exports.pending_scope = None;
                    }
                    cx.notify();
                });
            }
            _ => {
                let _ = this.update(cx, |this, cx| {
                    if kind == FileChoice::Output {
                        this.exports.pending_scope = None;
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub(super) fn refresh(&mut self, trigger: PreviewTrigger, cx: &mut Context<Self>) {
        self.pending_preset_restore = None;
        self.preview.refresh(
            self.photos.get(self.selected).cloned(),
            self.photos.clone(),
            self.settings.render_settings(),
            trigger,
            cx,
        );
    }
    pub(super) fn open_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.export_dialog.is_some() || self.settings_dialog.is_some() {
            return;
        }
        let settings = &self.settings;
        let count = self.photos.len();
        let parent = cx.entity().downgrade();
        let previous_focus = window.focused(cx);
        let dialog = cx.new(|cx| ExportDialog::new(parent, settings, count, previous_focus, cx));
        window.focus(&dialog.read(cx).focus_handle);
        self.export_dialog = Some(dialog);
        cx.notify();
    }
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.export_dialog.is_some() || self.settings_dialog.is_some() {
            return;
        }
        let parent = cx.entity().downgrade();
        let previous_focus = window.focused(cx);
        let dialog =
            cx.new(|cx| SettingsDialog::new(parent, self.settings.language, previous_focus, cx));
        window.focus(&dialog.read(cx).focus_handle);
        self.settings_dialog = Some(dialog);
        cx.notify();
    }
    pub(super) fn export(&mut self, all: bool, cx: &mut Context<Self>) {
        if self.exports.is_running() || self.photos.is_empty() {
            return;
        }
        let Some(output) = self.settings.output.clone() else {
            self.status = tr!("status.choose_output");
            self.exports.pending_scope = Some(all);
            self.choose(FileChoice::Output, cx);
            cx.notify();
            return;
        };
        let paths = if all {
            self.photos.as_ref().clone()
        } else {
            vec![self.photos[self.selected].clone()]
        };
        self.status = tr!("status.export_start", total = paths.len());
        self.exports
            .start(paths, output, self.settings.render_settings(), cx);
        cx.notify();
    }
    pub(super) fn command(&mut self, command: Command, cx: &mut Context<Self>) {
        if self.export_dialog.is_some() || self.settings_dialog.is_some() {
            return;
        }
        let pending_restore = self.pending_preset_restore.take();
        match command {
            Command::Import => {
                self.choose(FileChoice::Photos, cx);
                return;
            }
            Command::Choose(kind) => {
                self.choose(kind, cx);
                return;
            }
            Command::Cancel => {
                self.exports.cancel();
                self.status = tr!("status.stopping");
                cx.notify();
                return;
            }
            Command::Expand(section) => {
                match section {
                    PanelSection::Styles => self.styles_open = !self.styles_open,
                    PanelSection::Composition => self.composition_open = !self.composition_open,
                    PanelSection::Layout => self.layout_open = !self.layout_open,
                    PanelSection::Text => self.text_open = !self.text_open,
                }
                cx.notify();
                return;
            }
            Command::Select(index) => {
                if index >= self.photos.len() {
                    cx.notify();
                    return;
                }
                self.selected = index;
            }
            Command::Clear => {
                Arc::make_mut(&mut self.photos).clear();
                self.thumbnails.clear();
                self.filmstrip_scroll.set_offset(point(px(0.), px(0.)));
                self.selected = 0;
            }
            Command::Remove => {
                if !self.photos.is_empty() {
                    let removed = Arc::make_mut(&mut self.photos).remove(self.selected);
                    self.thumbnails.remove(&removed);
                    self.selected = self.selected.min(self.photos.len().saturating_sub(1));
                }
            }
            Command::Ratio(ratio) => self.settings.style.ratio = ratio,
            Command::Background(bg) => self.settings.style.background = bg,
            Command::Color(color) => {
                self.settings.style.color = color;
                self.settings.style.text_color =
                    if color.iter().map(|v| u32::from(*v)).sum::<u32>() > 384 {
                        [45, 45, 43]
                    } else {
                        [244, 244, 240]
                    };
                self.sync_inputs(cx);
            }
            Command::Font(family) => {
                self.settings.style.font = None;
                self.settings.style.font_family = Some(family.into());
            }
            Command::Align(align) => self.settings.style.align = StyleNumber::Align.clamp(align),
            Command::CaptionDefault => {
                let defaults = Settings::default();
                self.settings
                    .apply_caption_template(&defaults.caption_template(String::new()));
                self.sync_inputs(cx);
            }
            Command::TemplateApply(index) => {
                if let Some(template) = self.settings.caption_templates.get(index).cloned() {
                    self.settings.apply_caption_template(&template);
                    self.sync_inputs(cx);
                }
            }
            Command::TemplateSave => {
                let name = self.template_name.read(cx).value().trim().to_owned();
                if name.is_empty() {
                    self.status = tr!("errors.template_name");
                } else {
                    let template = self.settings.caption_template(name.clone());
                    if let Some(existing) = self
                        .settings
                        .caption_templates
                        .iter_mut()
                        .find(|t| t.name == name)
                    {
                        *existing = template;
                    } else {
                        self.settings.caption_templates.push(template);
                    }
                    self.status = tr!("status.template_saved", name = name);
                }
                cx.notify();
                return;
            }
            Command::TemplateDelete(index) => {
                if index < self.settings.caption_templates.len() {
                    self.settings.caption_templates.remove(index);
                    self.status = tr!("status.template_deleted");
                }
                cx.notify();
                return;
            }
            Command::Toggle(key) => match key {
                ToggleField::Shadow => self.settings.style.shadow = !self.settings.style.shadow,
                ToggleField::FontBold => {
                    self.settings.style.font_bold = !self.settings.style.font_bold
                }
                ToggleField::FontItalic => {
                    self.settings.style.font_italic = !self.settings.style.font_italic
                }
            },
            Command::Number(field, delta) => field.adjust(&mut self.settings.style, delta),
            Command::Reset => {
                if self.settings.preset_modified() {
                    self.pending_preset_restore = self.settings.selected_preset;
                    self.styles_open = true;
                } else if self.settings.selected_preset.is_none() {
                    self.status = tr!("status.custom_style");
                } else {
                    self.status = tr!("status.preset_default");
                }
                cx.notify();
                return;
            }
            Command::Preset(index) => {
                if let Some(preset) = StylePreset::ALL.get(index).copied() {
                    if self.settings.selected_preset == Some(preset) {
                        cx.notify();
                        return;
                    }
                    self.settings.switch_preset(preset);
                    self.sync_inputs(cx);
                }
            }
            Command::RequestPresetRestore(index) => {
                if let Some(preset) = StylePreset::ALL.get(index).copied()
                    && self.settings.selected_preset == Some(preset)
                    && self.settings.preset_modified()
                {
                    self.pending_preset_restore = Some(preset);
                }
                cx.notify();
                return;
            }
            Command::ConfirmPresetRestore(index) => {
                let preset = StylePreset::ALL.get(index).copied();
                if let Some(preset) = preset.filter(|preset| {
                    pending_restore == Some(*preset)
                        && self.settings.selected_preset == Some(*preset)
                        && self.settings.preset_modified()
                }) {
                    self.settings.restore_preset(preset);
                    self.sync_inputs(cx);
                } else {
                    cx.notify();
                    return;
                }
            }
            Command::CancelPresetRestore => {
                cx.notify();
                return;
            }
        }
        self.refresh(PreviewTrigger::Immediate, cx);
    }
}
