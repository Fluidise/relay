# Décision de conception : Compatibilité Relay avec la sélection explicite de politique de CLINT

## 1. Contexte et Mandat initial

- **Objectif** : Aligner les exemples de configuration, la documentation, le script de hook de développement local et le test d'intégration opt-in de Relay avec la nouvelle exigence de politique explicite de `fluidise-clint` (`--config <path>` ou `--defaults`), introduite pour éliminer la découverte implicite de fichiers dans Git.
- **Mode d'exécution retenu** : **Mode 1 (Décisions + documentation)**. Analyse approfondie, validation des mécanismes et formalisation des décisions sans altérer le code source, les scripts ou la suite de tests durant cette étape.
- **Mandat et orientations approuvées** :
  - Relay demeure un orchestrateur universel et agnostique : aucun moteur linter n'est embarqué ni codé en dur dans le binaire Relay.
  - Privilégier l'option `--config fluidise.toml` dans les exemples de Relay et dans le test d'intégration (permettant la cohabitation des sections `[hooks]` et `[commit]` dans le même fichier), tout en documentant explicitement l'alternative `--defaults` pour les usages sans configuration personnalisée.
  - Sauvegarde de la synthèse décisionnelle dans `docs/decisions/clint-compatibility.md`.

---

## 2. Analyse technique et faits vérifiés

1. **Évolution de l'interface CLI de CLINT (`fluidise-clint`)** :
   - *Ancien comportement* : `fluidise-clint --file <path>` découvrait automatiquement `fluidise.toml` à la racine Git si l'option `--config` n'était pas fournie.
   - *Nouveau comportement (depuis `feat!: require explicit policy selection`)* :
     `fluidise-clint (--file <path> | --stdin) (--config <path> | --defaults) [--human]`
     L'absence conjointe de `--config` et `--defaults` provoque immédiatement une erreur opérationnelle (code de sortie 2) avec le diagnostic :
     `{"code":"cli.configuration.required","message":"Select --config or --defaults.","source":"arguments"}`.
2. **Architecture du runner de Relay (`src/runner.rs`)** :
   - Relay prépare et exécute les commandes déclarées dans `[[hooks.commit-msg]]` en remplaçant uniquement l'argument entier `{message-file}` par le chemin absolu du fichier de message temporaire.
   - Les commandes sont exécutées avec le répertoire de travail positionné à la racine du dépôt géré (`repo.root`).
   - Le moteur de Relay est d'ores et déjà 100 % compatible : il accepte n'importe quelle liste d'arguments sans validation sémantique des options propres aux linters externes. Aucune modification du code source de Relay (`src/`) n'est requise.
3. **Points de contact impactés dans Relay** :
   - `README.md` (section *Configuring commit-msg*) : présente un exemple avec `args = ["--file", "{message-file}"]`.
   - `docs/usage.md` (section *Configure the chain*) : présente un exemple similaire sans argument de politique.
   - `scripts/install-hooks.sh` (ligne 34) : exécute `"$LINTER" --file "$COMMIT_MSG_FILE" --human` lors du hook Git local.
   - `tests/lifecycle.rs` (test `real_clint_through_git_and_direct_execution`) : configure `args = ["--file", "{message-file}"]` et teste le comportement réel face à une configuration invalide.

---

## 3. Arbre de décisions et arbitrage

### Décision 1 : Périmètre d'intervention (Moteur vs Documentation & Tests)
- **Options examinées** :
  - *Option A* : Détecter si `spec.program` correspond à `fluidise-clint` et injecter automatiquement `--defaults` ou `--config`.
  - *Option B (Retenue)* : Conserver Relay strictement agnostique et cantonner la mise à jour à la documentation, aux exemples, au script utilitaire et aux tests.
- **Justification** : L'Option A introduirait un couplage fort et une exception spécifique dans un orchestrateur générique de hooks, violant les garanties architecturales de Relay. L'Option B maintient la transparence : l'utilisateur configure explicitement les arguments de l'outil qu'il orchestre.

### Décision 2 : Convention pour les exemples de configuration (`README.md`, `docs/usage.md`)
- **Options examinées** :
  - *Option A* : Remplacer par `args = ["--file", "{message-file}", "--defaults"]`.
  - *Option B (Retenue)* : Remplacer par `args = ["--file", "{message-file}", "--config", "fluidise.toml"]`, assorti d'une note explicative sur `--defaults`.
- **Justification** : Dans un projet adoptant Fluidise, `fluidise.toml` centralise naturellement la configuration de Relay (`[hooks]`) et les règles de validation de message (`[commit]`). Pointer vers `"fluidise.toml"` (qui se résout depuis la racine du dépôt géré) est le cas d'usage cible le plus représentatif et permet de personnaliser les types et règles. Mentionner `--defaults` offre l'alternative clé en main pour un démarrage sans section de règles.

### Décision 3 : Alignement du test d'intégration opt-in (`tests/lifecycle.rs`)
- **Options examinées** :
  - *Option A* : Utiliser `--defaults`.
  - *Option B (Retenue)* : Utiliser `args = ["--file", "{message-file}", "--config", "fluidise.toml"]`.
- **Justification** : Le test valide trois phases successives :
  1. Commit valide (`feat: exercise relay`) $\rightarrow$ validation réussie (`exit_code: 0`).
  2. Message invalide (`invalid message`) $\rightarrow$ message rejeté (`exit_code: 1`).
  3. Configuration altérée (`[commit] version=999`) $\rightarrow$ erreur opérationnelle (`exit_code: 2`).
  Si le test utilisait `--defaults`, CLINT ignorerait le fichier `fluidise.toml` et la troisième phase échouerait (CLINT retournerait le code 1 pour message invalide au lieu du code 2 pour configuration invalide). L'usage de `--config fluidise.toml` est donc strictement indispensable à la cohérence du contrat de test.

### Décision 4 : Alignement du hook de développement local (`scripts/install-hooks.sh`)
- **Options examinées** :
  - *Option A (Retenue)* : Remplacer l'appel par `"$LINTER" --file "$COMMIT_MSG_FILE" --defaults --human`.
  - *Option B* : Ajouter un fichier `fluidise.toml` à la racine de Relay et appeler `--config`.
- **Justification** : Le dépôt `relay` lui-même n'a pas besoin d'un fichier `fluidise.toml` pour son fonctionnement interne (il n'impose pas de règles de commit exotiques en dehors des types conventionnels standard). L'emploi de `--defaults` rend le script de hook de développement autonome et résilient sans générer de fichier de configuration superflu.

---

## 4. Spécification détaillée des modifications (pour Mode 2 ultérieur)

### 4.1. `README.md`
```toml
# Configuration recommandée dans README.md
[hooks]
version = 1
timeout-seconds = 60

[[hooks.commit-msg]]
id = "commit-policy"
program = "fluidise-clint"
args = ["--file", "{message-file}", "--config", "fluidise.toml"]
modifies-message = false
```
*Note documentaire à ajouter* : Mentionner que `fluidise-clint` exige un choix explicite de politique (`--config <chemin>` pour cibler un fichier de règles ou `--defaults` pour utiliser les règles intégrées sans fichier).

### 4.2. `docs/usage.md`
Même mise à jour de la table `[[hooks.commit-msg]]` avec explication du rôle de `--config fluidise.toml` et mention du mode `--defaults`.

### 4.3. `scripts/install-hooks.sh`
```bash
# Ligne 34
if ! "$LINTER" --file "$COMMIT_MSG_FILE" --defaults --human; then
```

### 4.4. `tests/lifecycle.rs`
```rust
// Ligne 498
fs::write(
    f.root.join("fluidise.toml"),
    format!(
        "[hooks]\nversion=1\n[[hooks.commit-msg]]\nid='commit-policy'\nprogram={}\nargs=['--file','{{message-file}}','--config','fluidise.toml']\n",
        json!(binary)
    ),
).unwrap();
```

---

## 5. Critères observables et preuves de validation obtenues (Mode 2)

Les validations suivantes ont été exécutées avec succès et constituent la preuve formelle de conformité :
1. **Conformité statique** :
   - `cargo fmt --all -- --check` : Succès (code 0, aucun reformatage nécessaire).
   - `cargo clippy --all-targets -- -D warnings` : Succès (code 0, aucun avertissement).
2. **Suite de tests complète** :
   - `cargo test --all-targets` : 8 unitaire + 5 CLI + 62 cycle de vie = **75 tests passés avec succès**, 0 échec, 1 ignoré (test réel clint opt-in).
3. **Test d'intégration opt-in avec le binaire CLINT réel récent** :
   - Commande exécutée :
     `FLUIDISE_CLINT_BIN="/Users/acrazie/.gemini/antigravity/worktrees/lint/optimize_tool_benchmark_performance/target/release/fluidise-clint" cargo test --test lifecycle real_clint_through_git_and_direct_execution -- --ignored`
   - Résultat : **1 test passé avec succès** (durée : 1.10s) :
     - Phase 1 : Exécution réelle via le hook Git Relay avec commit valide (`feat: exercise relay`) $\rightarrow$ validation CLINT réussie (code 0).
     - Phase 2 : Validation d'un message invalide via `f.run()` $\rightarrow$ rejet conforme par CLINT (code 1).
     - Phase 3 : Validation avec configuration corrompue `[commit] version=999` $\rightarrow$ erreur opérationnelle interceptée et retournée par CLINT (code 2).
4. **Validation syntaxique des scripts** :
   - `bash -n scripts/install-hooks.sh` : Succès (code 0).
   - `git diff --check` : Succès (code 0, aucun problème d'espace blanc).

---

## 6. Bilan d'exécution du mandat

- **Statut** : Complété (Mode 2 : Décisions + Documentation + Implémentation + Tests).
- **Fichiers modifiés** :
  - `README.md` : alignement de l'exemple de configuration et documentation de `--config fluidise.toml` et `--defaults`.
  - `docs/usage.md` : alignement de l'exemple et précision des prérequis d'arguments de CLINT.
  - `scripts/install-hooks.sh` : ajout de l'argument explicite `--defaults` à l'invocation de CLINT.
  - `tests/lifecycle.rs` : configuration de `args = ['--file', '{message-file}', '--config', 'fluidise.toml']` dans le test réel.
  - `docs/decisions/clint-compatibility.md` : centralisation de l'analyse, des décisions et des preuves de validation.
- **Résultat** : Compatibilité totale et vérifiée avec la politique explicite de sélection de CLINT, sans régression ni modification du moteur de Relay.
