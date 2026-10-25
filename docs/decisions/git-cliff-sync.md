# Décision d'Architecture : Alignement de la Configuration `git-cliff` de Relay sur Clint

- **Statut** : Implémenté et Validé (Mode 2 : Décisions + Documentation + Implémentation Locale)
- **Date** : 2026-10-25
- **Projet** : `fluidise-relay` (`Fluidise/relay`)
- **Auteur / Mandat** : Mandat autonome d'architecture (`/autonomous-design-acrazie`)
- **Dépôt source de parité** : `fluidise-clint` (`Fluidise/clint`)

---

## 1. Mandat & Contexte Initial

### 1.1. Objectif du Mandat
Auditer et re-checker la configuration de changelog et de release notes outillée avec [`git-cliff`](https://github.com/orhun/git-cliff) récemment stabilisée et déployée sur le dépôt `fluidise-clint` (`Fluidise/clint`), déterminer les écarts avec la configuration initiale de `relay`, et formaliser les décisions architecturales et techniques pour synchroniser le dépôt `fluidise-relay`.

### 1.2. Mode d'Exécution & Autorisation
- **Mode d'exécution sélectionné** : **Mode 2 (Décisions + Documentation + Implémentation Locale)**, expressément autorisé par l'utilisateur ("go mode 2").
- **Périmètre d'action** :
  1. Exploration comparative exhaustive des configurations de `clint` (sur sa branche principale `main` post-release v0.1.0) et des artefacts préliminaires de `relay`.
  2. Test de simulation locale avec le moteur binaire `git-cliff 2.14.1` sur l'historique réel de `relay`.
  3. Formalisation de la décision, de l'arbre d'options et des blueprints techniques complets dans ce document central.
  4. Création de la configuration racine [`cliff.toml`](../../cliff.toml) adaptée pour `Fluidise/relay`.
  5. Création du pipeline GitHub Actions [`.github/workflows/release.yml`](../../.github/workflows/release.yml) avec injection de `GITHUB_TOKEN`.
  6. Génération initiale du fichier [`CHANGELOG.md`](../../CHANGELOG.md) via `git-cliff`.
  7. Mise à jour de la documentation opérationnelle dans [`docs/git-workflow.md`](../git-workflow.md).
  8. Validation locale complète de la conformité du code et des tests (`cargo fmt`, `clippy`, `cargo test`).
- **Limites strictes** : Aucune action de livraison Git distante (pas de `git commit`, `git push` ou ouverture de PR), conformément aux règles de la compétence d'arrêter les opérations aux modifications locales testées. Le commit et le push restent du ressort exclusif d'une tâche de delivery dédiée.

### 1.3. Contraintes Projet Existantes
1. **Politique Git & Gouvernance** : Trunk-Based Development avec base `main` ([docs/git-workflow.md](../git-workflow.md)).
2. **Conventions de Commit** : Conventional Commits vérifiés en local par `fluidise-clint`.
3. **Règle absolue de non-co-autorat** : Interdiction formelle de toute mention de type `Co-authored-by: Claude` (ou tout co-auteur automatique), y compris dans les workflows CI/CD et les messages de bot.

---

## 2. Analyse Comparative Détaillée (Clint vs Relay)

### 2.1. Trajectoire Historique des Deux Dépôts
1. **Phase 1 (Relay Initial)** : Une première configuration `cliff.toml` et un workflow de release avaient été ébauchés sur la branche `add_git_cliff_changelog` de Relay ([`docs/decisions/git-cliff-changelog.md`](file:///Users/acrazie/.gemini/antigravity/worktrees/relay/add_git_cliff_changelog/docs/decisions/git-cliff-changelog.md)).
2. **Phase 2 (Clint Adopte et Améliore)** : Clint a repris cette structure lors de la préparation de sa release `v0.1.0` (PR `#38` et `#39`), puis l'a considérablement perfectionnée lors des PR `#40` et `#41` suite aux constats opérationnels sur le comportement des GitHub Releases :
   - Intégration du composant natif d'avatars de contributeurs GitHub (`Box-footer`).
   - Déclaration explicite du remote GitHub (`[remote.github]`).
   - Filtrage des bots (`[bot]`, ex: `renovate[bot]`).
   - Nettoyage de la liste textuelle redondante `### 👥 Contributors`.
3. **Phase 3 (Besoin Actuel de Relay)** : Répercuter ces avancées et retours d'expérience sur Relay afin d'assurer une expérience homogène et sans faille à travers tous les dépôts de l'organisation Fluidise.

---

### 2.2. Tableau Comparatif des Configurations `cliff.toml`

| Composant | Relay (Configuration Initiale `add_git_cliff_changelog`) | Clint (Configuration Validée Finale `main`) | Écart & Décision pour Relay |
| :--- | :--- | :--- | :--- |
| **Section `[remote.github]`** | Absente en début de configuration | Déclarée en tête (`owner = "Fluidise"`, `repo = "clint"`) | **À intégrer impérativement** avec `repo = "relay"`. Permet à `git-cliff` de contextualiser les appels d'API et la construction d'URLs distantes. |
| **Section `❤️ New Contributors`** | Absente | Présente avec boucle Tera filtrant `is_first_time: true` et excluant `ending_with("[bot]")` | **À intégrer**. Déclenche l'affichage natif des avatars de contributeurs dans GitHub Releases. |
| **Section `👥 Contributors` (texte)** | Absente | Ajoutée dans PR #40 puis explicitement retirée dans PR #41 | **À omettre**. La mention `@username` dans les commits / New Contributors suffit pour les avatars ; la liste textuelle alourdit inutilement le changelog. |
| **Préprocesseur de PRs** | `\(#([0-9]+)\)` -> lien vers `Fluidise/relay` | `\(#([0-9]+)\)` -> lien vers `Fluidise/clint` | **Conserver** la cible `Fluidise/relay`. |
| **Emojis et Groupes de commits** | 12 groupes ordonnés (`🚀 Features`, `🐛 Bug Fixes`, `⚡ Perf`, `🔄 Refactor`, `📚 Docs`, `🎨 Style`, `🧪 Test`, `📦 Build`, `👷 CI`, `◀️ Reverts`, `⚙️ Misc`, `💥 Breaking`) | Identiques | **Identique** (parfaite cohérence inter-dépôts). |
| **Format du Scope** | `**{{ commit.scope }}:** ` | `**{{ commit.scope }}:** ` | **Identique** (gras + deux-points). |
| **Filtrage Chores & CI** | Ignore `^chore\(release\)` et `^chore.*[sS]kip [cC][iI]` | Ignore `^chore\(release\)` et `^chore.*[sS]kip [cC][iI]` | **Identique**. |
| **Liens Compare (Footer)** | Keep a Changelog standard (`compare/v...` et `tree/v...`) | Keep a Changelog standard (`compare/v...` et `tree/v...`) | **Identique** (avec repo adapté). |

---

### 2.3. Découverte Critique lors de l'Audit : Le Piège de l'API GitHub et du Rate Limiting

Lors de nos tests d'exécution de `git-cliff 2.14.1` sur Relay avec le fichier temporaire simulant Clint :
1. **Échec en mode anonyme sans token** :
   ```text
   thread 'main' panicked at git-cliff-core/src/changelog.rs:508:18:
   Could not get github metadata: HttpClientError(reqwest::Error { kind: Status(403, None), url: "https://api.github.com/repos/Fluidise/relay/commits?per_page=100&page=0" })
   ```
   *Cause* : Dès lors que `[remote.github]` est présent dans `cliff.toml`, `git-cliff` tente d'interroger l'API REST de GitHub pour récupérer les métadonnées (auteurs, PRs, primo-contributeurs). Sans authentification, GitHub applique une limite d'adresse IP de 60 requêtes/heure, déclenchant une erreur 403 immédiate.
2. **Succès avec authentification** :
   Avec `GITHUB_TOKEN=$(gh auth token)`, l'exécution réussit instantanément (code 0), produit le changelog complet et résout toutes les métadonnées distantes.
3. **Succès en mode offline** :
   Avec l'argument `git-cliff --offline`, l'exécution réussit sans toucher au réseau et sans nécessiter de token, en s'appuyant uniquement sur les métadonnées Git locales.

**Conséquence pour le workflow GitHub Actions (`.github/workflows/release.yml`)** :
Dans le workflow initial de Relay (`add_git_cliff_changelog`), l'action `orhun/git-cliff-action@v4` recevait `GITHUB_REPO: ${{ github.repository }}` mais **ne recevait pas** `GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}` ! Sans cette variable d'environnement, l'appel de l'action en CI risquait d'échouer ou de ne jamais renseigner les contributeurs.
Dans Clint, l'environnement CI transmet explicitement :
```yaml
env:
  OUTPUT: RELEASE_NOTES.md
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```
Cette correction est impérative pour le workflow de Relay.

---

## 3. Arbre de Décision (Frontière de Décision Résolue)

### D1 : Adoption Intégrale de la Configuration `cliff.toml` de Clint pour Relay
- **Option A (Recommandée & Retenue)** : Dupliquer la configuration `cliff.toml` de Clint en ajustant les références de `Fluidise/clint` vers `Fluidise/relay`. Inclure `[remote.github]` au début, la section `❤️ New Contributors` filtrant les bots, et exclure la liste textuelle `👥 Contributors`.
- **Option B (Statu quo Relay initial)** : Conserver le fichier sans `[remote.github]` ni gestion des contributeurs.
  *Rejetée* : Prive Relay des avatars visuels sur les releases GitHub et crée une asymétrie injustifiée avec Clint.

### D2 : Robustesse de l'Orchestration CI/CD (`.github/workflows/release.yml`)
- **Option A (Retenue)** :
  1. Déclenchement sur tag `v*` ou `workflow_dispatch`.
  2. Injection obligatoire de `GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}` dans chaque étape `orhun/git-cliff-action@v4`.
  3. Génération des release notes avec l'argument `--current --strip header` (ou `--latest --strip header`).
  4. Publication de la release via `softprops/action-gh-release@v2`.
  5. Régénération du `CHANGELOG.md` complet et synchronisation sur `main` avec le commit `chore(release): update CHANGELOG.md for ${{ github.ref_name }} [skip ci]`.
- **Option B (Adoption du workflow complet de build/distrib de Clint)** :
  *Rejetée pour le moment* : Clint possède une matrice multi-plateforme lourde (5 runners, tests de distribution Python, licences, etc.). Pour Relay (en cours de finalisation V1), le workflow dédié Release & Changelog est parfaitement dimensionné et indépendant.

### D3 : Recommandations pour l'Utilisation Locale de `git-cliff`
- Documenter dans `docs/git-workflow.md` que les développeurs exécutant `git-cliff` en local doivent soit :
  - Disposer de `GITHUB_TOKEN` dans leur environnement (ou via `gh auth token`).
  - Ou ajouter le flag `--offline` s'ils souhaitent tester le rendu sans dépendance réseau (`git cliff --offline`).

---

## 4. Spécification Technique Prête pour Implémentation (Blueprint Mode 2)

Ces artefacts représentent le code définitif prêt à être écrit lors d'une future phase d'implémentation.

### 4.1. Blueprint : `cliff.toml` pour Relay

```toml
# git-cliff ~ configuration file
# https://git-cliff.org/docs/configuration

[remote.github]
owner = "Fluidise"
repo = "relay"

[changelog]
# A Tera template to be rendered as the changelog's header.
header = """
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

"""
# A Tera template to be rendered for each release in the changelog.
body = """
{% if version -%}
    ## [{{ version | trim_start_matches(pat="v") }}] - {{ timestamp | date(format="%Y-%m-%d") }}
{% else -%}
    ## [Unreleased]
{% endif -%}

{% for group, commits in commits | group_by(attribute="group") %}
    ### {{ group | striptags | trim | upper_first }}
    {% for commit in commits %}
        - {% if commit.scope %}**{{ commit.scope }}:** {% endif %}\
            {% if commit.breaking %}[**breaking**] {% endif %}\
            {{ commit.message | split(pat="\n") | first | trim }}\
            {% if commit.id %} ([{{ commit.id | truncate(length=7, end="") }}](https://github.com/Fluidise/relay/commit/{{ commit.id }})){% endif %}\
    {% endfor %}
{% endfor %}
{% if github and github.contributors | length != 0 -%}
{% set new_contributors = github.contributors | filter(attribute="is_first_time", value=true) -%}
{% if new_contributors | length != 0 %}
### ❤️ New Contributors
{% for contributor in new_contributors -%}
  {%- if not contributor.username is ending_with("[bot]") %}
* @{{ contributor.username }} made their first contribution\
  {%- if contributor.pr_number %} in [#{{ contributor.pr_number }}](https://github.com/Fluidise/relay/pull/{{ contributor.pr_number }}){% endif %}
  {%- endif %}
{% endfor %}
{% endif -%}
{%- endif %}
\n"""
# A Tera template to be rendered as the changelog's footer.
footer = """
{% for release in releases -%}
    {% if release.version and release.previous and release.previous.version -%}
        [{{ release.version | trim_start_matches(pat="v") }}]: https://github.com/Fluidise/relay/compare/{{ release.previous.version }}...{{ release.version }}
    {% elif release.version -%}
        [{{ release.version | trim_start_matches(pat="v") }}]: https://github.com/Fluidise/relay/tree/{{ release.version }}
    {% elif release.previous and release.previous.version -%}
        [unreleased]: https://github.com/Fluidise/relay/compare/{{ release.previous.version }}...HEAD
    {% endif -%}
{% endfor %}
<!-- generated by git-cliff -->
"""
# Remove leading and trailing whitespaces from the changelog's body.
trim = true

[git]
# Parse commits according to the conventional commits specification.
conventional_commits = true
# Exclude commits that do not match the conventional commits specification.
filter_unconventional = false
split_commits = false
commit_preprocessors = [
  # Transform PR references like (#123) into links to Fluidise/relay PRs
  { pattern = '\(#([0-9]+)\)', replace = "([#${1}](https://github.com/Fluidise/relay/pull/${1}))" },
]
commit_parsers = [
  { message = "^feat", group = "<!-- 0 -->🚀 Features" },
  { message = "^fix", group = "<!-- 1 -->🐛 Bug Fixes" },
  { message = "^perf", group = "<!-- 2 -->⚡ Performance" },
  { message = "^refactor", group = "<!-- 3 -->🔄 Refactor" },
  { message = "^docs", group = "<!-- 4 -->📚 Documentation" },
  { message = "^style", group = "<!-- 5 -->🎨 Styling" },
  { message = "^test", group = "<!-- 6 -->🧪 Testing" },
  { message = "^build", group = "<!-- 7 -->📦 Build & Packaging" },
  { message = "^ci", group = "<!-- 8 -->👷 Continuous Integration" },
  { message = "^revert", group = "<!-- 9 -->◀️ Reverts" },
  { message = "^chore\\(release\\)", skip = true },
  { message = "^chore.*[sS]kip [cC][iI]", skip = true },
  { message = "^chore", group = "<!-- 10 -->⚙️ Miscellaneous Tasks" },
  { body = ".*BREAKING CHANGE.*", group = "<!-- 00 -->💥 Breaking Changes" },
]
protect_breaking_commits = false
filter_commits = false
tag_pattern = "v[0-9].*"
topo_order = false
sort_commits = "oldest"
```

---

### 4.2. Blueprint : `.github/workflows/release.yml` pour Relay

```yaml
name: Release & Changelog

on:
  push:
    tags:
      - "v*.*.*"
      - "v*"
  workflow_dispatch:
    inputs:
      tag:
        description: "Release tag to process (optional; defaults to HEAD ref)"
        required: false
        type: string

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

permissions:
  contents: write

jobs:
  release:
    name: Publish Release & Update Changelog
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository (full history)
        uses: actions/checkout@v4
        with:
          fetch-depth: 0
          token: ${{ secrets.GITHUB_TOKEN }}

      - name: Generate Release Notes for GitHub Release
        id: git-cliff-release
        uses: orhun/git-cliff-action@v4
        with:
          config: cliff.toml
          args: --latest --strip header
        env:
          OUTPUT: RELEASE_NOTES.md
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Publish GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          name: Release ${{ github.ref_name }}
          body: ${{ steps.git-cliff-release.outputs.content }}
          draft: false
          prerelease: ${{ contains(github.ref_name, '-') }}
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Prepare main branch for changelog update
        run: |
          set -euo pipefail
          git fetch origin main
          git checkout -B main origin/main

      - name: Generate complete CHANGELOG.md
        uses: orhun/git-cliff-action@v4
        with:
          config: cliff.toml
          args: --verbose
        env:
          OUTPUT: CHANGELOG.md
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Commit and push updated CHANGELOG.md to main
        run: |
          set -euo pipefail
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"

          if git status --porcelain CHANGELOG.md | grep -q "CHANGELOG.md"; then
            echo "CHANGELOG.md modified. Committing and pushing back to main..."
            git add CHANGELOG.md
            git commit -m "chore(release): update CHANGELOG.md for ${{ github.ref_name }} [skip ci]"
            if ! git push origin main; then
              echo "::warning::Direct push to main was declined by GitHub branch protection. Ensure GITHUB_TOKEN has push permissions or configure a release PR bypass."
              exit 1
            fi
          else
            echo "CHANGELOG.md is already up to date."
          fi
```

---

### 4.3. Simulation Locale Validée du Rendu `CHANGELOG.md` sur l'Historique Actuel de Relay

Le test exécuté sur les commits actuels de Relay (`ee5cadd`, `738baa5`, `8366e13`, `b4967ad`) avec cette configuration produit la sortie suivante :

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### 🚀 Features

- implement agent-first commit-msg manager ([#2](https://github.com/Fluidise/relay/pull/2)) ([8366e13](https://github.com/Fluidise/relay/commit/8366e13d17eeb623a759c780ca0296f151a4b9e0))

### ⚙️ Miscellaneous Tasks

- initialize repository structure and tooling ([ee5cadd](https://github.com/Fluidise/relay/commit/ee5cadd599a72f80833bc47f9f94fecc9956f0a6))

### 📚 Documentation

- add branded relay README banner ([#1](https://github.com/Fluidise/relay/pull/1)) ([738baa5](https://github.com/Fluidise/relay/commit/738baa5dc6168c68e9bbe83c6aab2b7fe704f759))
- update README for commit-msg manager ([#3](https://github.com/Fluidise/relay/pull/3)) ([b4967ad](https://github.com/Fluidise/relay/commit/b4967ad4ea35db5022b8ee6edc3037ba0691a8fa))

<!-- generated by git-cliff -->
```

---

## 5. Critères de Succès & Preuves Observables

1. **Stricte parité de formatage** : Emojis, Keep a Changelog, SemVer, conversion des issues/PRs `(#N)` en hyperliens valides vers `Fluidise/relay`.
2. **Déclenchement des avatars GitHub Releases** : Présence de la section `❤️ New Contributors` filtrant les bots pour alimenter `mentions_count` et activer le carrousel visuel sans alourdir le texte markdown de release.
3. **Résilience CI/CD** : Injection explicite de `GITHUB_TOKEN` évitant le rate limiting de l'API GitHub.
4. **Intégrité du code local** : Aucune régression sur les suites de vérification du projet (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`).

---

## 6. Bilan & Statut Final du Mandat

- **Statut** : **COMPLÉTÉ (Mode 2 : Décisions + Documentation + Implémentation Locale)**.
- **Fichiers créés & modifiés** :
  - [`cliff.toml`](../../cliff.toml) : Configuration racine unifiée de `git-cliff` adaptée à `Fluidise/relay`.
  - [`.github/workflows/release.yml`](../../.github/workflows/release.yml) : Pipeline CI/CD GitHub Actions pour push de tag (`v*`) et `workflow_dispatch` avec `GITHUB_TOKEN`.
  - [`CHANGELOG.md`](../../CHANGELOG.md) : Changelog initial généré et synchronisé à la racine du dépôt.
  - [`docs/git-workflow.md`](../git-workflow.md) : Ajout de la section `6. Release Management & Changelog Automation`.
  - [`docs/decisions/git-cliff-sync.md`](./git-cliff-sync.md) : Rapport d'architecture et enregistrement de la décision.
- **Validations exécutées** :
  - `git-cliff --output CHANGELOG.md` : génération conforme aux spécifications Keep a Changelog.
  - `cargo fmt --all -- --check` : code Rust 100% formaté.
  - `cargo clippy --all-targets -- -D warnings` : 0 avertissement.
  - `cargo test --all-targets` : 75 tests passés avec succès.
- **Prochaine étape (hors périmètre autonome)** : Revue des modifications locales par l'utilisateur et livraison Git (commit / PR).
