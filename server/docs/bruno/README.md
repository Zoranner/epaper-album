# 电子相册服务端 Bruno 接口文档

该目录是服务端接口的 Bruno 集合，覆盖管理台、设备同步和 sprite 生成接口。

## 使用方式

1. 在 Bruno 中打开 `server/docs/bruno` 目录。
2. 选择 `Development` 环境。
3. 按本地服务配置调整环境变量。
4. 先执行 `鉴权认证/登录`，把返回的 `jwtToken` 写入环境变量。

## 环境变量

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `baseUrl` | `http://localhost:3000` | 服务端地址 |
| `jwtToken` | 空 | 登录请求写入的管理员 `jwtToken` |

其他测试数据直接写在请求文件中。
