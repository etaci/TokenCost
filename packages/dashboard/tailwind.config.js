/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './src/pages/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    './src/app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        background: '#F7F4EF',
        surface: '#FBF9F5',
        border: '#E7E1D7',
        foreground: '#1F2421',
        muted: '#5C635D',
        accent: {
          DEFAULT: '#C4612F',
          hover: '#A94E22',
          light: '#F2E3D6',
        },
      },
    },
  },
  plugins: [],
}
