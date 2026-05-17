import { defineConfig } from "vitepress";

const GITHUB = "https://github.com/Jannchie/arthash";

export default defineConfig({
  title: "arthash",
  description:
    "A compact placeholder-image hash — 17 B to 400 B per image, enough to render a recognisable preview while the real image loads.",
  cleanUrls: true,
  lastUpdated: true,

  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: "/logo.svg" }],
    ["meta", { name: "theme-color", content: "#0284c7" }],
  ],

  themeConfig: {
    logo: "/logo.svg",
    socialLinks: [{ icon: "github", link: GITHUB }],
    search: { provider: "local" },
  },

  locales: {
    root: {
      label: "English",
      lang: "en-US",
      themeConfig: {
        nav: [
          { text: "Guide", link: "/guide/introduction" },
          { text: "API", link: "/api/typescript" },
          { text: "Benchmark", link: "/benchmark/" },
          {
            text: "v0.1.2",
            items: [
              { text: "Changelog", link: `${GITHUB}/blob/main/CHANGELOG.md` },
              { text: "npm", link: "https://www.npmjs.com/package/arthash" },
              { text: "PyPI", link: "https://pypi.org/project/arthash/" },
              { text: "crates.io", link: "https://crates.io/crates/arthash" },
            ],
          },
        ],
        sidebar: {
          "/guide/": [
            {
              text: "Getting Started",
              items: [
                { text: "Introduction", link: "/guide/introduction" },
                { text: "Installation", link: "/guide/installation" },
                { text: "Basic Usage", link: "/guide/basic-usage" },
                { text: "Modes & Codecs", link: "/guide/modes" },
                { text: "Palettes", link: "/guide/palettes" },
              ],
            },
          ],
          "/api/": [
            {
              text: "API Reference",
              items: [
                { text: "TypeScript", link: "/api/typescript" },
                { text: "Python", link: "/api/python" },
                { text: "Rust", link: "/api/rust" },
              ],
            },
          ],
          "/benchmark/": [
            {
              text: "Benchmarks",
              items: [
                { text: "Overview", link: "/benchmark/" },
                { text: "DCT vs thumbhash", link: "/benchmark/dct" },
                { text: "Shape vs sqip", link: "/benchmark/shape" },
                { text: "Visual Quality", link: "/benchmark/quality" },
              ],
            },
          ],
        },
        editLink: {
          pattern: `${GITHUB}/edit/main/docs/site/:path`,
          text: "Edit this page on GitHub",
        },
        footer: {
          message: "Released under the MIT License.",
          copyright: "Copyright © 2024–present Jianqi Pan",
        },
        outline: { label: "On this page" },
        docFooter: { prev: "Previous page", next: "Next page" },
        lastUpdatedText: "Last updated",
      },
    },

    zh: {
      label: "简体中文",
      lang: "zh-CN",
      link: "/zh/",
      themeConfig: {
        nav: [
          { text: "指南", link: "/zh/guide/introduction" },
          { text: "API", link: "/zh/api/typescript" },
          { text: "Benchmark", link: "/zh/benchmark/" },
          {
            text: "v0.1.2",
            items: [
              { text: "更新日志", link: `${GITHUB}/blob/main/CHANGELOG.md` },
              { text: "npm", link: "https://www.npmjs.com/package/arthash" },
              { text: "PyPI", link: "https://pypi.org/project/arthash/" },
              { text: "crates.io", link: "https://crates.io/crates/arthash" },
            ],
          },
        ],
        sidebar: {
          "/zh/guide/": [
            {
              text: "快速开始",
              items: [
                { text: "简介", link: "/zh/guide/introduction" },
                { text: "安装", link: "/zh/guide/installation" },
                { text: "基础用法", link: "/zh/guide/basic-usage" },
                { text: "模式与 Codec", link: "/zh/guide/modes" },
                { text: "调色板", link: "/zh/guide/palettes" },
              ],
            },
          ],
          "/zh/api/": [
            {
              text: "API 参考",
              items: [
                { text: "TypeScript", link: "/zh/api/typescript" },
                { text: "Python", link: "/zh/api/python" },
                { text: "Rust", link: "/zh/api/rust" },
              ],
            },
          ],
          "/zh/benchmark/": [
            {
              text: "性能测试",
              items: [
                { text: "总览", link: "/zh/benchmark/" },
                { text: "DCT vs thumbhash", link: "/zh/benchmark/dct" },
                { text: "Shape vs sqip", link: "/zh/benchmark/shape" },
                { text: "画质对比", link: "/zh/benchmark/quality" },
              ],
            },
          ],
        },
        editLink: {
          pattern: `${GITHUB}/edit/main/docs/site/:path`,
          text: "在 GitHub 上编辑此页",
        },
        footer: {
          message: "基于 MIT 协议发布。",
          copyright: "Copyright © 2024-至今 Jianqi Pan",
        },
        outline: { label: "页面导航" },
        docFooter: { prev: "上一页", next: "下一页" },
        lastUpdatedText: "最后更新",
        returnToTopLabel: "回到顶部",
        sidebarMenuLabel: "菜单",
        darkModeSwitchLabel: "主题",
        lightModeSwitchTitle: "切换到浅色模式",
        darkModeSwitchTitle: "切换到深色模式",
      },
    },

    ja: {
      label: "日本語",
      lang: "ja-JP",
      link: "/ja/",
      themeConfig: {
        nav: [
          { text: "ガイド", link: "/ja/guide/introduction" },
          { text: "API", link: "/ja/api/typescript" },
          { text: "ベンチマーク", link: "/ja/benchmark/" },
          {
            text: "v0.1.2",
            items: [
              { text: "変更履歴", link: `${GITHUB}/blob/main/CHANGELOG.md` },
              { text: "npm", link: "https://www.npmjs.com/package/arthash" },
              { text: "PyPI", link: "https://pypi.org/project/arthash/" },
              { text: "crates.io", link: "https://crates.io/crates/arthash" },
            ],
          },
        ],
        sidebar: {
          "/ja/guide/": [
            {
              text: "はじめに",
              items: [
                { text: "概要", link: "/ja/guide/introduction" },
                { text: "インストール", link: "/ja/guide/installation" },
                { text: "基本的な使い方", link: "/ja/guide/basic-usage" },
                { text: "モードと Codec", link: "/ja/guide/modes" },
                { text: "パレット", link: "/ja/guide/palettes" },
              ],
            },
          ],
          "/ja/api/": [
            {
              text: "API リファレンス",
              items: [
                { text: "TypeScript", link: "/ja/api/typescript" },
                { text: "Python", link: "/ja/api/python" },
                { text: "Rust", link: "/ja/api/rust" },
              ],
            },
          ],
          "/ja/benchmark/": [
            {
              text: "ベンチマーク",
              items: [
                { text: "概要", link: "/ja/benchmark/" },
                { text: "DCT vs thumbhash", link: "/ja/benchmark/dct" },
                { text: "Shape vs sqip", link: "/ja/benchmark/shape" },
                { text: "画質比較", link: "/ja/benchmark/quality" },
              ],
            },
          ],
        },
        editLink: {
          pattern: `${GITHUB}/edit/main/docs/site/:path`,
          text: "このページを GitHub で編集",
        },
        footer: {
          message: "MIT ライセンスのもとリリース。",
          copyright: "Copyright © 2024-現在 Jianqi Pan",
        },
        outline: { label: "目次" },
        docFooter: { prev: "前のページ", next: "次のページ" },
        lastUpdatedText: "最終更新",
      },
    },
  },
});
