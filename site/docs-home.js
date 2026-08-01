// mdBook's header links to the book root, not to the site root, so the
// docs are a one-way trip without this: put "margin" back in the top bar
// pointing at the landing page. Six lines beats forking the theme.
(function () {
  var bar = document.querySelector(".menu-title");
  if (!bar) return;
  var home = document.createElement("a");
  home.href = "../";
  home.textContent = "▌margin";
  home.className = "margin-home";
  home.setAttribute("aria-label", "Margin home");
  bar.textContent = "";
  bar.appendChild(home);
})();
