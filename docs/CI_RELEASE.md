# CI 发布流程

## 触发条件

GitHub Actions 工作流 `.github/workflows/release.yml` 仅在 **GitHub Release 发布（published）** 时触发，普通 push / PR 不会触发构建。

触发方式有两种：

- 在 GitHub 仓库页面手动创建 Release 并发布。
- 通过 `gh` CLI 创建：

  ```bash
  gh release create v0.1.0 --title "v0.1.0" --notes "发布说明..."
  ```

## 构建架构

当前支持以下 Linux 目标平台：

| target                     | 说明               |
| -------------------------- | ------------------ |
| `x86_64-unknown-linux-gnu` | 常见 x86_64 桌面   |
| `aarch64-unknown-linux-gnu` | ARM64 设备（如树莓派、Apple Silicon Linux 虚拟机） |

每次构建产出 `devtools-runner-<target>.tar.gz`，内含：

- `devtools-runner`（二进制）
- `org.kde.devtools.desktop`（KRunner 元数据）
- `org.kde.devtools.service`（D-Bus 激活配置）
- `install.sh`（安装脚本）

产物通过 `gh release upload` 自动上传到对应 Release，覆盖已存在的同名文件（`--clobber`）。

## 版本号

遵循 [语义化版本 SemVer](https://semver.org/lang/zh-CN/)：`MAJOR.MINOR.PATCH`

| 版本号 | 变更类型                       |
| ------ | ----------------------------- |
| MAJOR  | 不兼容的 API 变更               |
| MINOR  | 向后兼容的功能新增               |
| PATCH  | 向后兼容的 bug 修复              |

- `Cargo.toml` 中的 `version` 字段为单一事实来源。
- 创建 Release 时，tag 名称使用 `v` 前缀，如 `v0.1.0`。
- 每次发布前需手动更新 `Cargo.toml` 中的版本号并提交。

## 发布步骤

1. 更新 `Cargo.toml` 的 `version` 字段。
2. 更新 `README.md`（如有必要）。
3. 提交并推送版本号变更：

   ```bash
   git add Cargo.toml README.md
   git commit -m "release: bump to v0.1.0"
   git push
   ```

4. 创建 Release：

   ```bash
   gh release create v0.1.0 \
     --title "v0.1.0" \
     --notes "$(cat docs/CHANGELOG.md 2>/dev/null || echo '初始发布')"
   ```

5. 等待 CI 构建完成，产物会自动上传到 Release。
