# Shape 模式 vs sqip

arthash 的 shape 模式（`CIRCLE`、`TRIANGLE`、`SQUARE`、`RECT`、`ROTATED_RECT`）对标 [sqip](https://github.com/axe312ger/sqip) 的 `primitive` 插件：爬山法叠加 N 个几何原语拟合原图，输出 SVG。arthash 更快、更小、且能在浏览器内跑。

## 编码时间（JS，baseline = sqip-node）

arthash 用积分图 + SSE 增量更新，搜索成本随 `n` 亚线性。sqip 是线性 + IPC 绑定（每多一个原语都要重新爬山一遍，调用 Go 二进制）。

| 实现                           |      n=12 (倍率) |      n=24 (倍率) |       n=64 (倍率) |
| ------------------------------ | ---------------: | ---------------: | ----------------: |
| arthash · ts TRIANGLE          | 5.1 ms (**56×**) | 7.9 ms (**56×**) | 15.2 ms (**67×**) |
| arthash · ts CIRCLE            |     5.3 ms (54×) |     7.2 ms (62×) |     15.5 ms (66×) |
| sqip · primitive-triangle @0.3 |           284 ms |           446 ms |           1015 ms |

差距**随 `n` 拉大**——n=12 时快 56×，n=64 时快 67×。

## 输出大小

| 实现                           |       n=12 (倍率) |        n=24 (倍率) |        n=64 (倍率) |
| ------------------------------ | ----------------: | -----------------: | -----------------: |
| arthash · ts CIRCLE            | 53 B (**16× 小**) | 102 B (**15× 小**) | 267 B (**14× 小**) |
| arthash · ts TRIANGLE          |     77 B (11× 小) |     150 B (10× 小) |      395 B (9× 小) |
| sqip · primitive-triangle @0.3 |             842 B |             1482 B |             3650 B |

sqip 直接输出完整 SVG 字符串，每个原语都带 `<polygon points="..." fill="..." ... />` 标记。arthash 把几何打成紧凑的位流，没有标记开销，只在渲染时才转成 SVG。

## 各自的适用场景

|                       | sqip (`primitive`)        | arthash (shape 模式)     |
| --------------------- | ------------------------- | ------------------------ |
| 运行时                | Node + Go 子进程          | 纯 wasm（浏览器 / Node） |
| 部署形态              | **仅构建期**              | 构建期或请求期           |
| 输出                  | 直接 SVG 字符串           | 紧凑 hash → 按需渲染 SVG |
| 输出大小              | 800 B – 4 kB              | 50 B – 400 B             |
| n=64 编码时间         | ~1 s                      | ~15 ms                   |
| 每形状颜色位宽        | 24 (truecolour) + alpha   | 16 (RGB-565) 或 4 (调色板) |

sqip 适合两端不可控、要直接拿 SVG 字符串的场景；arthash 适合两端可控、想要更小载荷或在浏览器编码的场景。

## 对比说明

- sqip 使用 `primitive-triangle` 插件、`progressive: 0.3` 默认配置——与插件官方示例一致。
- arthash 使用默认 `Codec::triangle(n)` / `Codec::circle(n)`（RGB-565 颜色，3 bit alpha）；切换到调色板模式更小，切换到 RGB-888 每形状多 ~8 bit。
- 所有 sqip / arthash 输出都按同一 `viewBox` 尺寸渲染，便于直接比较硬盘大小。
