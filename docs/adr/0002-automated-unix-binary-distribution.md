# ADR 0002: Automatisation du Packaging et de la Distribution Binaire Multi-Cibles Unix

* **Statut :** Accepté
* **Date :** 2026-10-25
* **Pilier / Tier :** Pilier 3 (Build & Packaging) / Tier 2 (Montée majeure / Distribution)

---

## 1. Contexte

`fluidise-relay` est un orchestrateur de crochets Git écrit en Rust, conçu pour une exécution déterministe, sûre et ultra-rapide (<1 ms) au sein d'environnements développeur et d'agents IA autonomes.

Jusqu'alors, l'installation de Relay nécessitait une compilation locale depuis les sources (`cargo build --release`), imposant aux environnements hôtes ou conteneurisés de disposer d'une chaîne d'outils Rust complète (MSRV 1.99.1, Cargo, compilateur LLVM).

Dans le cadre du Pilier 3 (Build & Packaging), une solution de distribution binaire automatisée multi-cibles doit être mise en place pour fournir des artefacts prêts à l'emploi.

---

## 2. Invariants et Contraintes Système

1. **Exclusion stricte de Windows :**
   Le moteur d'orchestration (`src/runner.rs`, `src/lifecycle.rs`) exploite des primitives système POSIX pures (`posix_spawn`, groupes de processus `setpgid`, masques de signaux, verrous consultatifs `flock`, descripteurs de fichiers Unix). Le support Windows n'est pas supporté par conception.
2. **Support Prioritaire Unix (Linux & macOS) :**
   Les cibles visées couvrent les environnements de développement et serveurs modernes :
   - `x86_64-unknown-linux-gnu` (Linux glibc standard)
   - `x86_64-unknown-linux-musl` (Linux binaire statique sans dépendance glibc)
   - `aarch64-unknown-linux-gnu` (Linux ARM64 / Graviton / conteneurs Apple Silicon)
   - `aarch64-unknown-linux-musl` (Linux ARM64 statique)
   - `x86_64-apple-darwin` (macOS Intel)
   - `aarch64-apple-darwin` (macOS Apple Silicon)
3. **Traçabilité et Intégrité :**
   Chaque archive binaire `.tar.gz` doit être accompagnée de son condensat cryptographique SHA-256 individuel et consolidé dans un manifeste `SHA256SUMS.txt`.

---

## 3. Décision et Alternatives Évaluées

### Alternative A : Publication unique sur crates.io (`cargo install`)
* **Évaluation :** Permet une installation via `cargo install fluidise-relay`.
* **Motif du rejet :** Nécessite toujours la présence du compilateur Rust sur la machine hôte et impose un temps de compilation à l'installation, incompatible avec l'initialisation instantanée d'environnements éphémères d'agents.

### Alternative B : Image Docker universelle
* **Évaluation :** Distribuer le binaire au sein d'une image OCI conteneurisée.
* **Motif du rejet :** Relay orchestre les crochets Git locaux d'un dépôt hôte. Un wrapper Docker introduit une latence d'initialisation prohibitive pour un crochet `commit-msg` et complique la gestion des permissions et montages de volumes.

### Décision Retenue
Déclarer les métadonnées de distribution dans [`Cargo.toml`](../../Cargo.toml) (`[package.metadata.dist]`) et scaffolder un workflow dédié [`.github/workflows/release.yml`](../../.github/workflows/release.yml) déclenché sur tag Git (`v*`) ou déclenchement manuel. Ce workflow :
1. Compile les binaires natifs et statiques pour les 6 cibles Unix (avec `cross` pour les architectures croisées Linux musl/aarch64).
2. Empaquette les binaires avec `README.md` et `LICENSE`.
3. Génère les condensats SHA256 et publie la GitHub Release avec la CLI officielle `gh`.

---

## 4. Conséquences

- **Disponibilité :** Les agents et développeurs peuvent télécharger directement le binaire correspondant à leur architecture sans dépendance vers Rust.
- **Sécurité et intégrité :** Vérification immédiate de la chaîne d'approvisionnement grâce aux empreintes SHA256.
- **Portabilité :** Les cibles musl garantissent un fonctionnement universel sur distributions Linux allégées (Alpine, conteneurs minimaux).

---

## 5. Preuves de Validation

- Validation syntaxique et schema `Cargo.toml` : `cargo check --locked --all-targets` vert.
- Validation YAML du workflow : conforme à GitHub Actions avec actions épinglées par SHA complet.
- Lints et formatage : `cargo fmt` et `cargo clippy` validés sans avertissement.


## Addendum du 30 octobre 2026 : publication 0.1.0

Le [mandat de publication](../decisions/release-0.1.0.md) conserve les six cibles
Unix, ajoute crates.io et le tap Homebrew macOS comme canaux complémentaires, et
remplace la recette `cross` par des builds sur runners natifs. Le workflow
réutilisable est la source effective de la matrice ; les métadonnées cargo-dist
non utilisées sont retirées. Les archives incluent désormais provenance,
inventaire vérifié et notices des tiers. La release ne met plus à jour le
changelog par push direct et ne remplace pas d'assets publiés.

La compatibilité de chaque cible doit être prouvée par l'exécution distante du
binaire extrait. Les seules validations statiques historiques de cette ADR ne
constituent pas une preuve de publication ou de fonctionnement multi-plateforme.
