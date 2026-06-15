import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Santui',
  description: 'A modular TUI framework for Rust, built on Ratatui',
  themeConfig: {
    siteTitle: 'Santui',
    nav: [
      { text: 'Guide', link: '/guide/' },
      { text: 'API', link: '/api/' },
    ],
    sidebar: {
      '/guide/': [
        {
          text: 'Guide',
          items: [
            { text: 'What is Santui?', link: '/guide/what-is-santui' },
            { text: 'Getting Started', link: '/guide/' },
          ],
        },
      ],
    },
    socialLinks: [
      { icon: 'github', link: 'https://github.com/sonyarianto/santui' },
    ],
    footer: {
      message: 'Built with Rust and Ratatui',
    },
  },
})
