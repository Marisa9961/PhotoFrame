# 印框 · PhotoFrame

使用 Rust + GPUI 构建的本地照片装裱工具，为照片添加留白、柔焦背景和拍摄信息。

![印框界面](docs/screenshot.png)

## 功能

- 氛围、留白、暗房、拍立得四种预设，各自记忆调整，支持确认后恢复默认。
- 自定义画布比例、留白、背景、圆角和投影，实时预览效果。
- 读取 EXIF 拍摄信息，支持自定义字体、文字及模板。
- 单张或批量导出 JPEG、PNG、WebP，同名文件自动编号，保留原图。
- 英文与简体中文界面，默认跟随系统语言。

## 效果示例

| 氛围 | 暗房 |
| --- | --- |
| ![氛围相框：柔焦背景与圆角照片](docs/DSC_6558_frame.jpg) | ![暗房相框：深色边框与浅色拍摄信息](docs/DSC_7421_frame.jpg) |
| 将照片色彩延展为柔焦背景，配合圆角与轻投影，让画面自然融入相框。适合花卉、人像等色彩丰富的照片。 | 以深色边框衬托画面，搭配浅色拍摄信息，突出光影层次。适合夜景、城市和暗调作品。 |

| 留白 | 拍立得 |
| --- | --- |
| ![留白相框：浅色窄边与简洁排版](docs/DSC_7154_frame.jpg) | ![拍立得相框：加宽底部留白](docs/DSC_6055_frame.jpg) |
| 用简洁的浅色留白分隔照片与周围环境，让构图和主体更清楚。底部保留拍摄信息，适合风景与日常记录。 | 加宽底部留白，呈现拍立得相纸般的比例。可以写下拍摄信息或一句短文，记录照片之外的故事。 |

## 构建与运行

目前支持并验证的平台为 **Windows x64**。需要 Rust stable（支持 edition 2024）、Visual Studio C++ 桌面工具链及 Windows SDK。

```powershell
cargo build --release --locked
.\target\release\photo-frame.exe
```

GPUI 需要 Windows SDK 中的 `fxc.exe`。若无法自动找到，请将其完整路径设置为环境变量 `GPUI_FXC_PATH`。

启动后拖入照片，选择风格、调整文字，再点击“导出照片”。支持导入 JPEG、PNG、WebP、TIFF 和 BMP；暂不支持 RAW／HEIC。

## 开发

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

## 发布

推送与 Cargo 版本一致的标签（当前为 `v1.0.0`），或在 GitHub Actions 中手动运行 **Windows draft release**。工作流会测试并构建 Windows x64 版本，将程序与许可证打包为 ZIP，上传到草稿 Release，确认后可手动发布。

## 许可

© Marisa9961 · [MIT License](LICENSE) · [第三方许可与署名](THIRD_PARTY_NOTICES.md)
