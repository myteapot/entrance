# Troubleshooting Guide

## 1. Tauri 自动更新 (Auto Updater) 报错闪退 / JSON 解析错误

**现象描述**：
运行集成了 `tauri-plugin-updater` 的 Entrance 时，刚打开（或点击检查更新）瞬间弹出一个报错对话框（"检查或安装更新失败: 未知错误..."），并且日志可能提示无法解析 JSON。

**根本原因**：
Entrance 的 `updater.json` 配置指向了私有部署的 GitLab Raw URL (例如 `http://server:9311/pub/entrance/-/raw/main/updater.json`)。当代码库或 GitLab 实例未完全对外公开时，GitLab 会拦截未经身份验证（无 Cookie/Token）的 HTTP 请求，并返回 **HTTP 302 重定向** 到 `/users/sign_in` 登录页面。
Tauri Updater 预期收到 JSON，但却收到了 302/HTML 页面，导致 JSON 解析崩溃并抛出未知错误（Unknown Error）。

**解决排查步骤**：

要让 Tauri Updater 能够在客户端免密抓取更新包，必须确保 GitLab 的下载链路完全 Public（或者必须在此环节使用携带 Token 的 headers）：

1. **项目是否点选了“公开 (Public)”且点击了保存？**
   去具体项目 -> `Settings` -> `General` -> `Visibility, project features, permissions` 下把 `Project visibility` 改为 `Public`。**注意必须滚动到最底部点击“保存更改 (Save changes)”。**
2. **父级群组 (Group) 是否限制了权限？**
   在 GitLab 中，子项目的可见性不能高于其所在的父级 Group。如果 `pub` 这个 Group 是 Private 的，那么底下的 `entrance` 即使配置了 Public 也无法生效。你需要先将父 Group 设置为 Public，再将子项目设为 Public。
3. **全局权限是否开启了“需要登录才能访问公开项目”？**
   哪怕项目是彻底公开的，如果管理员在后台开启了全站封禁，依然会拦截请求。
   进入 `Admin Area (管理员区域)` -> `Settings (设置)` -> `General (常规)` -> 找到 `Sign-in restrictions (登录限制)`，从中**取消勾选**选项：`Require authentication to view public projects (要求身份验证以查看公共项目)`，然后保存。

*(注：为兼顾绝对的内网安全性与丝滑的自动更新，未来长期架构设计见 Linear 记录卡片 MYT-43，即通过 Vault 注入 Bot Token 执行 `check({ headers: { "PRIVATE-TOKEN": "..." } })`)*
