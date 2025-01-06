{
  ...
}:

{
  languages.rust = {
    enable = true;
  };

  git-hooks.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
  };

  dotenv = {
    enable = true;
    filename = ".env";
  };
}
