# Publication de Relay 0.1.0

## Mandat et périmètre

Le 30 octobre 2026, l'utilisateur a demandé la publication de `fluidise-relay`
sur crates.io et Homebrew, selon le modèle CLINT. Il a confirmé la version
`0.1.0`, la préparation et les tests, puis commit/push, PR, merge après CI verte,
tag, release GitHub, publication crates.io et formule dans
`Fluidise/homebrew-tap`. Il a également demandé une revue adversariale indépendante.
Cette autorisation ne couvre ni contournement des protections, ni modification
des paramètres ou des identifiants, ni installation de hooks.

Le mode de conception est « décisions, documentation et implémentation locale ».
La livraison Git et les publications sont des opérations distinctes couvertes
par la confirmation explicite ci-dessus. Les priorités sont l'intégrité, la
traçabilité, la simplicité et la cohérence avec CLINT. La logique de Relay et son
protocole d'approbation ne changent pas.

## Faits et décisions

- Base inspectée : `045ed6660ca9a77b1781ce8a7538694aca211ed6`, également `main`
  distant au début de la préparation. Le worktree était propre et détaché.
  Branche de travail : `chore/publish-relay-0.1.0`.
- Relay interdit actuellement Cargo publish et ne possède aucune release GitHub.
  Ne rien changer ne permet donc pas de satisfaire la demande.
- CLINT dispose maintenant d'une release GitHub `v0.1.0`. Sa matrice, son
  inventaire d'archives et ses notices runtime servent de modèle, pas de preuve
  de compatibilité de Relay. Les sources ont été lues sur GitHub ; les notices
  sont vérifiées par checksum et liées au commit exact du compilateur.
- Cargo devient publiable uniquement sur `crates-io`. Une liste explicite inclut
  le code, les tests Rust, le verrou, la licence et les instructions utilisateur.
  Les scripts et documents de maintenance ne sont pas nécessaires au paquet.
- Les six cibles Unix de l'ADR 0002 sont conservées, sans Windows. Les compilations
  utilisent des runners natifs Linux ARM64/x86-64 et macOS ARM64/Intel plutôt que
  `cross`, ce qui retire une dépendance de compilation et permet d'exécuter chaque
  binaire sur son architecture. GNU utilise glibc du runner Ubuntu 24.04 ; aucune
  compatibilité avec une glibc plus ancienne n'est promise. Les variantes musl
  sont destinées aux systèmes sans glibc.
- Un workflow réutilisable construit, inventorie et exécute les archives. La CI
  doit attendre cette matrice avant son job existant de merge automatique.
  La release réutilise exactement la même recette sur le commit du tag, pas les
  artefacts de la PR. Une publication existante n'est jamais écrasée.
- Le tag stable doit correspondre à Cargo et désigner un commit de `main`.
  La release vérifie les six cibles, les checksums internes, le commit source,
  le verrou et l'absence de modifications locales déclarée dans la provenance.
  Chaque archive porte son checksum externe ; un manifeste consolidé est publié.
- Comme CLINT 0.1.0, les binaires ne sont ni signés par l'éditeur ni notariés Apple.
  Les SHA-256 assurent l'intégrité relativement à une source de confiance, pas
  l'identité de l'éditeur. Aucun contournement des protections du système n'est fourni.
- Les notices des dépendances Cargo et du runtime Rust/LLVM sont conservées dans
  les archives et l'installation Homebrew. Les notices musl sont ajoutées aux
  variantes concernées. Les SDK et bibliothèques système ne sont pas redistribués.
- Homebrew suit le périmètre macOS ARM64/Intel du tap CLINT existant. La nouvelle
  formule utilise les URLs GitHub immuables et les SHA-256 des archives vérifiées,
  sans modifier la formule CLINT ni son hébergement actuel. Git est un prérequis
  explicite. La formule est répétée dans un préfixe temporaire avec le vrai DSL
  Homebrew sur les deux architectures, sans installation ou activation de hooks.
- crates.io reste une publication distincte, après validation du paquet exact.
  Aucun mécanisme d'authentification n'est inventé : les variables Cargo usuelles,
  les fichiers de credentials et les secrets du dépôt sont absents lors de
  l'inspection. L'upload est bloqué sans authentification existante autorisée.

## Critères observables

1. Format, Clippy et tests Rust obligatoires réussissent sur le candidat livré.
2. `cargo publish --dry-run --locked --registry crates-io` réussit ; le paquet
   contient toutes les sources nécessaires et aucun secret.
3. La revue indépendante accepte un snapshot identifié avant passage de la PR
   en ready. Toute correction matérielle doit être revue à nouveau.
4. Les six archives passent les tests de packaging, leur extraction contrôlée et
   l'exécution réelle d'un hook dans un dépôt temporaire. Un délégué introuvable
   échoue et le message n'est pas modifié.
5. La CI complète, y compris la distribution, réussit avant merge. Le tag désigne
   le commit validé de `main`. Les assets publics sont relus et vérifiés.
6. La formule Homebrew contient les checksums des assets publiés ; ses méthodes
   install/test sont exécutées avec le vrai DSL sur macOS ARM64 et Intel.
7. crates.io expose exactement `fluidise-relay` `0.1.0`, et la formule est
   accessible dans `Fluidise/homebrew-tap`. Une étape bloquée ne compte pas comme
   publication complète.

## Sources et limites

- [Cargo : publication](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [Cargo : manifeste](https://doc.rust-lang.org/cargo/reference/manifest.html)
- [Homebrew : formules](https://docs.brew.sh/Formula-Cookbook)
- [GitHub : workflows réutilisables](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows)
- [ADR 0002](../adr/0002-automated-unix-binary-distribution.md)
- [Utilisation et opérations de livraison](../distribution.md)

Les docs Cargo, Homebrew et GitHub ont été consultées via Context7. Les tests
locaux macOS ne prouvent pas le comportement Linux ou Intel : la matrice distante
est obligatoire. La revue recherche des défauts dans le changement ; elle n'est
ni une garantie d'absence de bogues ni un avis juridique.

## État de réalisation

La préparation locale est réalisée : métadonnées Cargo, packaging dérivé de
CLINT, workflows, génération et répétition de formule Homebrew, notices et
runbook. Aucun code de l'application n'a été modifié.

Preuves locales sur macOS ARM64 : format et Clippy réussis ; 84 tests Rust
réussis et un test d'intégration au vrai CLINT explicitement ignoré ; 17 tests
Python de packaging réussis ; Cargo dry-run a construit le paquet de 15 fichiers
sans upload ; le binaire release a exécuté le smoke test de cycle de vie et de
délégué indisponible dans un dépôt temporaire. Syntaxe Ruby de la formule générée
et du lanceur de répétition vérifiée. La répétition Homebrew effective et les
six plateformes restent à vérifier dans la CI distante. L'absence d'actionlint
local n'est pas assimilée à une validation par GitHub.

La revue indépendante et les opérations externes ne sont pas encore attestées
par ces preuves locales. La publication crates.io est bloquée par l'absence
d'authentification disponible. Les autres étapes indépendantes peuvent continuer. Le support Windows,
un installateur curl/npm, un nouveau domaine de packages, la signature et la
notarisation sont exclus de cette version.
