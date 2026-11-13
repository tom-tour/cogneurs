// Fixed address of the Cogneurs server.
// '' = same address as the page (when the server also serves the page).
window.COGNEURS_API = '';

// Where the home launcher publishes the server's current address
// (a GitHub gist, or any URL returning {"url": "https://..."}).
// When the server is off or unreachable, the page plays offline solo.
window.COGNEURS_DISCOVERY = 'https://api.github.com/gists/a06e4d37f026c071419b6dcbec01d11a';
