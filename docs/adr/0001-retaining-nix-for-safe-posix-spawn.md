# ADR 0001: Rétention de la Dépendance nix pour l'Invocation Sécurisée de posix_spawn

* **Statut :** Accepté
* **Date :** 2026-10-24
* **Pilier / Tier :** Pilier 6 (Gestion des dépendances) / Tier 2 (Arbitrage architectural)

---

## 1. Contexte

`fluidise-relay` est un orchestrateur autonome et sécurisé de crochets Git. Pour garantir la robustesse des exécutions concurrentes et la sécurité des processus, le projet applique l'invariable absolue suivante dans son manifeste `Cargo.toml` :

```toml
[lints.rust]
unsafe_code = "forbid"
```

Pour la quasi-totalité de ses opérations système (manipulation de descripteurs de fichiers, verrous consultatifs `flock`, attentes non bloquantes `waitpid`, signaux et scrutation `poll`), le projet utilise la caisse moderne [rustix](https://crates.io/crates/rustix) (`1.1.5`).

Néanmoins, pour l'apparition des processus délégués dans `src/runner.rs` (`spawn_managed`), le code fait appel à `posix_spawn`, `PosixSpawnFileActions` et `PosixSpawnAttr` via la dépendance [nix](https://crates.io/crates/nix) (`0.31.3`, avec `features = ["process", "signal"]`).

Dans le cadre de l'audit de modernisation, la possibilité d'éliminer `nix` pour n'utiliser que `rustix` ou la bibliothèque standard a été évaluée.

---

## 2. Décision et Alternatives Évaluées

### Alternative A : Remplacement par des appels directs à `libc::posix_spawn`
* **Évaluation :** Utiliser directement `libc` permettrait de retirer `nix` de l'arbre de dépendances.
* **Motif du rejet :** Les liaisons C directes requièrent des blocs `unsafe`. Or, le dépôt interdit formellement tout code non sécurisé (`unsafe_code = "forbid"`). Introduire des dérogations `unsafe` compromettrait l'assurance de sécurité mémoire du projet.

### Alternative B : Remplacement par `std::process::Command` et `CommandExt::pre_exec`
* **Évaluation :** La bibliothèque standard propose `std::os::unix::process::CommandExt`.
* **Motif du rejet :** L'assignation granulaire de descripteurs de fichiers au-dessus de la plage standard (FD > 3) et la reconfiguration des signaux par défaut nécessitent `pre_exec`. Cette fonction est intrinsèquement `unsafe` en Rust et notoirement dangereuse en présence de threads (risques de verrous orphelins et de deadlocks au niveau de l'allocateur mémoire).

### Alternative C : Remplacement par `rustix`
* **Évaluation :** Centraliser tous les appels système sous `rustix`.
* **Motif du rejet :** `rustix` a pour philosophie de dialoguer directement avec le noyau Linux/Unix via des appels système purs sans liaison libc. `posix_spawn` est une fonction de haut niveau de la libc standard POSIX et n'est pas exposée par `rustix`.

### Décision Retenue
**Conserver `nix` (version 0.31.3)** avec une configuration minimale (`features = ["process", "signal"]`). `nix` fournit une abstraction sûre et éprouvée (`safe Rust`) autour de `posix_spawn`, garantissant l'absence totale de code `unsafe` dans le dépôt tout en offrant le contrôle bas niveau requis sur les groupes de processus (`POSIX_SPAWN_SETPGROUP`) et les signaux (`POSIX_SPAWN_SETSIGDEF`).

---

## 3. Conséquences

- **Sécurité :** L'invariable `unsafe_code = "forbid"` est maintenue à 100% sur l'intégralité du code source.
- **Empreinte :** En restreignant les fonctionnalités de `nix` à `["process", "signal"]`, l'arbre de compilation reste léger et hermétique.
- **Process Safety :** Les processus enfants continuent d'être lancés dans leur propre groupe de processus, assurant une interruption propre (`SIGINT`/`SIGTERM`/`SIGKILL`) sans risque de fuite de sous-processus orphelins.

---

## 4. Preuves de Validation

- Vérification statique : `cargo clippy --all-targets -- -D warnings` exécuté sans avertissement.
- Vérification fonctionnelle : 84 tests automatisés validés vert (tests unitaires, CLI et 62 tests de recovery lifecycle couvrant interruptions, signaux et timeouts).
