# CodeConvoy homepage

Static project homepage for <https://chrislauinger77.github.io/code-convoy/>.
This is a separate marketing site; the desktop application remains native
Rust/egui. The site uses HTML/CSS, system fonts and existing repository artwork,
with no JavaScript, frontend dependencies, analytics or third-party asset requests.

## Preview locally

From the repository root:

```sh
python3 website/build.py
python3 -m http.server 8000 --bind 127.0.0.1 --directory dist/homepage
```

Open <http://127.0.0.1:8000/>. Re-run the staging command after edits; the server
can stay running. Stop the server with Ctrl+C when finished.

The staging script uses only Python's standard library and writes to the ignored
`dist/homepage/` directory. It copies an explicit list of public files, including
the original icon and screenshot from `assets/`, so those images need updating
in only one place. Relative asset URLs work under the `/code-convoy/` Pages path
as well as at the local preview root. Download links point to the latest release
instead of duplicating the application version.

## Publish with GitHub Pages

1. Commit these changes and push them to `main` in
   `ChrisLauinger77/code-convoy` when ready to publish.
2. In the repository's **Settings → Pages → Build and deployment**, set
   **Source** to **GitHub Actions**.
3. Run **Actions → Homepage → Run workflow** on `main` for the first deployment
   after enabling Pages (or rerun a failed initial deployment).
4. Wait for the `github-pages` deployment to succeed. The homepage will be at
   <https://chrislauinger77.github.io/code-convoy/>.

Subsequent pushes to `main` that change the website, its artwork or its workflow
deploy automatically. The workflow uploads only `dist/homepage/`. It uses
GitHub's Pages artifact and deployment actions with an environment and scoped
deployment permissions; no custom domain, token or hosting service is needed.
Manual runs from other branches cannot deploy.

See [GitHub's custom Pages workflow documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

Before publishing edits, inspect desktop and narrow mobile layouts, follow the
navigation and download links, and check keyboard focus. Product and platform
claims should remain consistent with the root README and release documentation.
