/** @type {import('tailwindcss').Config}*/
const config = {
	content: ['./src/**/*.{html,js,svelte,ts}'],

	theme: {
		extend: {}
	},

	plugins: [
		function({ addVariant }) {
			addVariant('child', '& > *');
        	addVariant('child-hover', '& > *:hover');
        	addVariant('child-focus', '& > *:focus');
		}
	]
};

module.exports = config;
