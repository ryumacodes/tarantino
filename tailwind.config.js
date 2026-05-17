/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        primary: '#0a0a0a',
        secondary: '#1a1a1a',
        tertiary: '#2a2a2a',
        border: '#3a3a3a',
        accent: '#ff6b6b',
        'accent-hover': '#ff5252',
      },
    },
  },
  plugins: [],
}