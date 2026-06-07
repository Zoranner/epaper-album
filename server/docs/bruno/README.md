# Epaper Album Server Bruno Collection

该目录是服务端接口的 Bruno 集合，覆盖管理台、设备同步和 sprite 生成接口。

## 使用方式

1. 在 Bruno 中打开 `server/docs/bruno` 目录。
2. 选择 `Local` 环境。
3. 按本地服务配置调整环境变量。
4. 先执行 `Auth/Login`，把返回的 token 写入 `adminToken`。

## 环境变量

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `baseUrl` | `http://localhost:3000` | 服务端地址 |
| `secretKey` | `local-secret-key` | 用户或设备权限密钥 |
| `adminUsername` | `admin` | 管理员账号 |
| `adminPassword` | `admin` | 管理员密码 |
| `adminToken` | 空 | 登录后填写或由登录请求写入 |
| `planId` | `1` | 更新、删除计划使用 |
| `imageSha256` | 空 | 更新备注、下载显示图使用 |
| `imageFile` | 空 | 上传图片使用的本地文件路径 |
| `imageRemark` | `海边晚风` | 上传或更新备注 |
| `spriteText` | `晚风和海` | sprite 生成文本 |
| `spriteType` | `caption` | `caption`、`date`、`status` 或 `notice` |
| `spriteEtag` | 空 | 测试 `If-None-Match` 时使用 |
