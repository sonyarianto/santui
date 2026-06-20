import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Santui',
  description: 'My terminal home base',
  cleanUrls: true,

  head: [
    ['link', { rel: 'canonical', href: 'https://santuiapp.vercel.app' }],
  ],

  appearance: 'dark',

  themeConfig: {
    siteTitle: 'Santui',
    nav: [
      { text: 'Guide', link: '/guide/' },
      { text: 'v0.1.8', link: 'https://github.com/sonyarianto/santui/releases/tag/v0.1.8' },
    ],
    sidebar: {
      '/guide/': [
        {
          text: 'Guide',
          items: [
            { text: 'What is Santui?', link: '/guide/what-is-santui' },
            { text: 'Getting Started', link: '/guide/' },
            { text: 'Themes', link: '/guide/themes' },
          ],
        },
      ],
    },
    socialLinks: [
      { icon: 'github', link: 'https://github.com/sonyarianto/santui' },
    ],
    footer: {
      message: 'Santui v0.1.8 — Copyright \u00a9 2026 <a href="https://github.com/sonyarianto" target="_blank" rel="noopener">Sony AK</a>',
    },
  },
})
