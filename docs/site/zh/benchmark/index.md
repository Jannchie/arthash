# Benchmarks

以下所有数据都在同一台机器上跑，输入为 100×100 RGB。

- **JS** 数据由 [`bench/js-cross/`](https://github.com/Jannchie/arthash/tree/main/bench/js-cross) 在 Node 22 下实测，原始 NDJSON 在 [`docs/benchmarks/js_cross_*.ndjson`](https://github.com/Jannchie/arthash/tree/main/docs/benchmarks)。
- **原生** 数据见 [`docs/benchmarks/CROSS_IMPL.md`](https://github.com/Jannchie/arthash/blob/main/docs/benchmarks/CROSS_IMPL.md)。
- 表格按速度排序，*vs baseline* 列 = baseline 时间 / 当前实现时间——大于 1 表示更快。

## 关键数据

| 场景                              | 最佳结果                                                |
| --------------------------------- | ------------------------------------------------------- |
| 24 B DCT encode (JS)              | 比 thumbhash-js **快 1.9×**                             |
| 24 B DCT encode (原生)            | 比最快的非 arthash 实现 **快 1.27×**                    |
| DCT decode @ 256 px (原生)        | 比 thumbhash-go @ 256 **快 5.9×**                       |
| TRIANGLE n=64 encode vs sqip (JS) | 编码 **快 67×**，输出 **小 9×**                         |
| 17 B 占位 PSNR                    | DCT @ 17 B = 23.3 dB —— 比 thumbhash @ 17 B 高 0.4 dB   |

更详细对比：

- [**DCT vs thumbhash**](./dct) —— 同字节预算下 arthash 的优势在哪。
- [**Shape vs sqip**](./shape) —— shape 模式如何在速度与大小上同时碾压 sqip。
- [**画质对比**](./quality) —— 各模式的 PSNR vs 字节数。

## arthash 为什么快

- **无 header 字节流。** 没有 magic number、没有模式标签、没有 bit width——每一个 bit 都是图像信息。
- **每通道独立 AC scale。** 每张图都有自己的量化表，但不为此付 header bit。
- **积分图 + SSE 增量搜索。** shape 模式搜索成本随 `n` 亚线性；sqip 是线性 + 每个原语 IPC 一次。
- **直接解码到目标尺寸。** thumbhash-js 只能输出 ~32 px；arthash IDCT 直接落在显示缓冲上。

完整算法故事见 [arthash 怎么省 bit](../guide/introduction#arthash-怎么省-bit)。

## 复现

```sh
# JS bench (Node 22)
cd bench/js-cross
pnpm install
pnpm run bench           # 产出 docs/benchmarks/js_cross_*.ndjson

# 原生 bench
pnpm run bench:rs        # Rust
pnpm run bench:py        # Python via PyO3
pnpm run bench:binding   # 跨绑定
```

以上数据同机重测波动 ±5% 以内。
