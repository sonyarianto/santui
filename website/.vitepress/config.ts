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
      message: 'Copyright 2026 Sony AK <sony@sony-ak.com>',
    },
  },
})
