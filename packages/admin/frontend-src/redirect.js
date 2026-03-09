if (!location.pathname.endsWith('/')) {
  location.replace(location.pathname + '/' + location.search + location.hash);
}
