// Tailwind v4 已移除 tailwindcss/colors 导出，不再从这里 require；
// 配置里并没有实际用到 colors，直接删除该 import。
module.exports = {
  content: [ './src/**/*.{js,jxs,vue}' ],
  darkMode: 'class', // or 'media' or 'class'
  theme: {
    fontFamily: {
      sans: ['Graphik', 'sans-serif'],
      serif: ['Merriweather', 'serif'],
    },
    extend: {
      spacing: {
        '128': '32rem',
        '144': '36rem',
      },
      borderRadius: {
        '4xl': '2rem',
      }
    }
  },
  variants: {
    extend: {
      borderColor: ['focus-visible'],
      opacity: ['disabled'],
    }
  },
  plugins: [
  ],
}
