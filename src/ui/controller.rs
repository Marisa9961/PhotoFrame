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
        let color_input = cx.new(|cx| TextInput::new(hex(settings.color), cx));
        let title_input = cx.new(|cx| TextInput::new(&settings.title, cx));
        let subtitle_input = cx.new(|cx| TextInput::new(&settings.subtitle, cx));
        let template_name = cx.new(|cx| TextInput::new(tr!("text.templates"), cx));
        let title_subscription = cx.subscribe(&title_input, |this, input, _: &Changed, cx| {
            this.settings.title = input.read(cx).value();
            this.refresh(PreviewTrigger::Input, cx);
        });
        let subtitle_subscription =
            cx.subscribe(&subtitle_input, |this, input, _: &Changed, cx| {
                this.settings.subtitle = input.read(cx).value();
                this.refresh(PreviewTrigger::Input, cx);
            });
        let subscription = cx.subscribe(&color_input, |this, input, _: &Changed, cx| {
            if let Some(color) = parse_color(&input.read(cx).value()) {
                this.settings.color = color;
                this.refresh(PreviewTrigger::Input, cx);
            } else {
                this.status = tr!("errors.color");
                cx.notify();
            }
        });
        let release_subscription = cx.on_release(|this, _| {
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
            photos: vec![],
            selected: 0,
            preview: None,
            preview_size: (0, 0),
            preview_target: (0, 0),
            thumbnails: BTreeMap::new(),
            filmstrip_scroll: ScrollHandle::new(),
            styles_open: true,
            composition_open: false,
            layout_open: false,
            text_open: false,
            export_dialog: None,
            settings_dialog: None,
            pending_export: None,
            pending_preset_restore: None,
            status,
            rendering: false,
            exporting: false,
            progress: (0, 0),
            failures: vec![],
            preview_state: Arc::new(Mutex::new(None)),
            generation: Arc::new(AtomicU64::new(0)),
            preview_task: None,
            cancel: Arc::new(AtomicBool::new(false)),
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
        self.color_input
            .update(cx, |input, cx| input.set(&hex(self.settings.color), cx));
        self.title_input
            .update(cx, |input, cx| input.set(&self.settings.title, cx));
        self.subtitle_input
            .update(cx, |input, cx| input.set(&self.settings.subtitle, cx));
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
                    self.photos.push(path);
                }
            } else {
                rejected += 1;
            }
        }
        let new_paths = self.photos[before..].to_vec();
        cx.spawn(async move |this, cx| {
            for path in new_paths {
                if this.update(cx, |this, _| this.photos.contains(&path)).ok() != Some(true) {
                    continue;
                }
                let source = path.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let photo = render::load_preview(&source, 240)?;
                        let mut pixels = photo.image.thumbnail(240, 120).to_rgba8();
                        for p in pixels.pixels_mut() {
                            p.0.swap(0, 2);
                        }
                        Ok::<_, anyhow::Error>(Arc::new(RenderImage::new(vec![image::Frame::new(
                            pixels,
                        )])))
                    })
                    .await;
                if this
                    .update(cx, |this, cx| {
                        if this.photos.contains(&path)
                            && let Ok(image) = result
                        {
                            this.thumbnails.insert(path, image);
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
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
    pub(super) fn choose(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        let response = cx.prompt_for_paths(PathPromptOptions {
            files: kind != "output",
            directories: kind == "output",
            multiple: kind == "photos",
            prompt: Some(
                match kind {
                    "photos" => tr!("prompts.photos"),
                    "font" => tr!("prompts.font"),
                    "logo" => tr!("prompts.logo"),
                    _ => tr!("prompts.output"),
                }
                .into(),
            ),
        });
        cx.spawn(async move |this, cx| match response.await {
            Ok(Ok(Some(paths))) => {
                let _ = this.update(cx, |this, cx| {
                    if kind == "photos" {
                        this.add(paths, cx);
                        return;
                    }
                    if let Some(path) = paths.into_iter().next() {
                        match kind {
                            "font" => this.settings.font = Some(path),
                            "logo" => this.settings.logo = Some(path),
                            _ => this.settings.output = Some(path),
                        }
                        if kind == "output" {
                            if let Some(all) = this.pending_export.take() {
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
                    if kind == "output" {
                        this.pending_export = None;
                    }
                    cx.notify();
                });
            }
            _ => {
                let _ = this.update(cx, |this, cx| {
                    if kind == "output" {
                        this.pending_export = None;
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub(super) fn refresh(&mut self, trigger: PreviewTrigger, cx: &mut Context<Self>) {
        self.pending_preset_restore = None;
        let request = PreviewRequest::new(self.generation.clone());
        // Dropping the task cancels pending timers instead of retaining one per keystroke.
        self.preview_task = None;
        let Some(path) = self.photos.get(self.selected).cloned() else {
            // Drop retained pixels without waiting on a background render's mutex.
            self.preview_state = Arc::new(Mutex::new(None));
            self.preview = None;
            self.thumbnails.retain(|path, _| self.photos.contains(path));
            self.rendering = false;
            cx.notify();
            return;
        };
        self.rendering = true;
        let target = self.preview_target;
        if target.0 == 0 || target.1 == 0 {
            self.rendering = false;
            cx.notify();
            return;
        }
        let settings = self.settings.clone();
        let paths = self.photos.clone();
        let preview_state = self.preview_state.clone();
        self.preview_task = Some(cx.spawn(async move |this, cx| {
            let delay = trigger.delay();
            if !delay.is_zero() {
                Timer::after(delay).await;
            }
            if !request.is_current() {
                return;
            }
            let worker_request = request.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut preview_state = preview_state
                        .lock()
                        .map_err(|_| anyhow::anyhow!(tr!("errors.renderer")))?;
                    if !worker_request.is_current() {
                        return Ok(None);
                    }
                    let state = preview_state.get_or_insert_with(PreviewState::default);
                    state.photos.retain(&paths);
                    let Some(mut pixels) = worker_request.run(
                        || state.photos.load(&path, target.0.max(target.1)),
                        |photo| {
                            state
                                .engine
                                .compose_cached_preview(&photo, &settings, target)
                        },
                    )?
                    else {
                        return Ok(None);
                    };
                    let dimensions = pixels.dimensions();
                    for pixel in pixels.pixels_mut() {
                        pixel.0.swap(0, 2);
                    }
                    let image = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
                    Ok::<_, anyhow::Error>(Some((image, dimensions)))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !request.is_current() {
                    return;
                }
                this.rendering = false;
                match result {
                    Ok(Some((image, dimensions))) => {
                        this.preview = Some(image);
                        this.preview_size = dimensions;
                    }
                    Err(e) => {
                        this.preview = None;
                        this.status = tr!("errors.preview", error = format!("{e:#}"));
                    }
                    Ok(None) => {}
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn open_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.export_dialog.is_some() || self.settings_dialog.is_some() {
            return;
        }
        let settings = self.settings.clone();
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
        if self.exporting || self.photos.is_empty() {
            return;
        }
        let Some(output) = self.settings.output.clone() else {
            self.status = tr!("status.choose_output");
            self.pending_export = Some(all);
            self.choose("output", cx);
            cx.notify();
            return;
        };
        let paths = if all {
            self.photos.clone()
        } else {
            vec![self.photos[self.selected].clone()]
        };
        let total = paths.len();
        self.progress = (0, total);
        self.exporting = true;
        self.failures.clear();
        self.cancel.store(false, Ordering::Relaxed);
        let cancel = self.cancel.clone();
        let settings = self.settings.clone();
        self.status = tr!("status.export_start", total = total);
        cx.spawn(async move |this, cx| {
            let started = std::time::Instant::now();
            let options = photo_frame::batch::BatchOptions::default();
            let worker_cancel = cancel.clone();
            let receiver = cx
                .background_executor()
                .spawn(async move {
                    photo_frame::batch::start(paths, output, settings, worker_cancel, options)
                })
                .await;
            let mut receiver = match receiver {
                Ok(receiver) => receiver,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.exporting = false;
                        this.status = tr!("errors.export_start", error = error);
                        cx.notify();
                    });
                    return;
                }
            };
            let (mut processed, mut success) = (0, 0);
            let mut active = std::collections::BTreeSet::new();
            let mut peak_concurrent = 0;
            loop {
                let (next, event) = cx
                    .background_executor()
                    .spawn(async move {
                        let event = receiver.recv();
                        (receiver, event)
                    })
                    .await;
                receiver = next;
                let Ok(event) = event else {
                    break;
                };
                if this
                    .update(cx, |this, cx| {
                        match event {
                            photo_frame::batch::BatchEvent::Configured { .. } => {}
                            photo_frame::batch::BatchEvent::Started { index, .. } => {
                                active.insert(index);
                                peak_concurrent = peak_concurrent.max(active.len());
                            }
                            photo_frame::batch::BatchEvent::Finished {
                                index,
                                source,
                                result,
                            } => {
                                active.remove(&index);
                                processed += 1;
                                this.progress = (processed, total);
                                match result {
                                    Ok(_) => {
                                        success += 1;
                                        let _ = this.flush_preferences();
                                    }
                                    Err(error) => this.failures.push(format!(
                                        "{}: {error:#}",
                                        source.file_name().unwrap_or_default().to_string_lossy()
                                    )),
                                }
                            }
                        }
                        this.status = tr!(
                            "status.export_progress",
                            processed = processed,
                            total = total,
                            success = success,
                            active = active.len(),
                            seconds = format!("{:.1}", started.elapsed().as_secs_f64())
                        );
                        cx.notify();
                    })
                    .is_err()
                {
                    cancel.store(true, Ordering::Relaxed);
                    return;
                }
            }
            let _ = this.update(cx, |this, cx| {
                this.exporting = false;
                if processed < total && !cancel.load(Ordering::Relaxed) {
                    this.failures
                        .push(tr!("errors.incomplete", count = total - processed));
                }
                this.status = tr!(
                    "status.export_done",
                    result = if cancel.load(Ordering::Relaxed) {
                        tr!("status.cancelled")
                    } else {
                        tr!("status.completed")
                    },
                    success = success,
                    failed = this.failures.len(),
                    peak = peak_concurrent,
                    seconds = format!("{:.1}", started.elapsed().as_secs_f64())
                );
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn command(&mut self, command: Command, cx: &mut Context<Self>) {
        if self.export_dialog.is_some() || self.settings_dialog.is_some() {
            return;
        }
        let pending_restore = self.pending_preset_restore.take();
        match command {
            Command::Import => {
                self.choose("photos", cx);
                return;
            }
            Command::Choose(kind) => {
                self.choose(kind, cx);
                return;
            }
            Command::Cancel => {
                self.cancel.store(true, Ordering::Relaxed);
                self.status = tr!("status.stopping");
                cx.notify();
                return;
            }
            Command::Expand(section) => {
                match section {
                    "styles" => self.styles_open = !self.styles_open,
                    "composition" => self.composition_open = !self.composition_open,
                    "layout" => self.layout_open = !self.layout_open,
                    _ => self.text_open = !self.text_open,
                }
                cx.notify();
                return;
            }
            Command::Select(index) => {
                self.selected = index;
                self.preview = None;
            }
            Command::Clear => {
                self.photos.clear();
                self.thumbnails.clear();
                self.filmstrip_scroll.set_offset(point(px(0.), px(0.)));
                self.selected = 0;
            }
            Command::Remove => {
                if !self.photos.is_empty() {
                    let removed = self.photos.remove(self.selected);
                    self.thumbnails.remove(&removed);
                    self.selected = self.selected.min(self.photos.len().saturating_sub(1));
                }
            }
            Command::Ratio(ratio) => self.settings.ratio = ratio,
            Command::Background(bg) => self.settings.background = bg,
            Command::Color(color) => {
                self.settings.color = color;
                self.settings.text_color = if color.iter().map(|v| u32::from(*v)).sum::<u32>() > 384
                {
                    [45, 45, 43]
                } else {
                    [244, 244, 240]
                };
                self.sync_inputs(cx);
            }
            Command::Font(family) => {
                self.settings.font = None;
                self.settings.font_family = Some(family.into());
            }
            Command::Align(align) => self.settings.align = align,
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
                "shadow" => self.settings.shadow = !self.settings.shadow,
                "font_bold" => self.settings.font_bold = !self.settings.font_bold,
                "font_italic" => self.settings.font_italic = !self.settings.font_italic,
                "uppercase" => self.settings.uppercase = !self.settings.uppercase,
                _ => self.settings.force_custom = !self.settings.force_custom,
            },
            Command::Number(key, delta) => {
                let (value, min, max) = match key {
                    "padding" => {
                        self.settings
                            .horizontal_padding
                            .get_or_insert(self.settings.padding);
                        (&mut self.settings.padding, 0, 30)
                    }
                    "horizontal_padding" => (
                        self.settings
                            .horizontal_padding
                            .get_or_insert(self.settings.padding),
                        0,
                        30,
                    ),
                    "footer" => (&mut self.settings.footer, 0, 40),
                    "radius" => (&mut self.settings.radius, 0, 20),
                    "blur" => (&mut self.settings.blur, 0, 100),
                    "background_dim" => (&mut self.settings.background_dim, 0, 90),
                    "font_size" => (&mut self.settings.font_size, 8, 80),
                    "line_gap" => (&mut self.settings.line_gap, 0, 40),
                    "max_edge" => (&mut self.settings.max_edge, 0, 16000),
                    _ => {
                        self.settings.quality =
                            (self.settings.quality as i32 + delta).clamp(1, 100) as u8;
                        cx.notify();
                        return;
                    }
                };
                *value = (*value as i32 + delta).clamp(min, max) as u32;
                if key == "max_edge" {
                    cx.notify();
                    return;
                }
            }
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
