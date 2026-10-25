# Codecov : pilote de couverture sur Relay

Date : 2026-10-25. Mode 1 initial, puis mode 2 autorisé par « mode 2 go ».

## Mandat et résultat

L'utilisateur déclare avoir installé l'intégration Codecov et lui avoir donné
accès à Relay et Clint. Il autorise une exploration autonome et la documentation
des décisions sur **Relay uniquement**, comme dépôt pilote avant un éventuel
portage sur Clint. Priorités acceptées : simplicité, aucune dépendance payante,
aucun secret exposé. Les choix d'intégration CI, de rapports et de seuils sont
délégués ; les décisions ci-dessous sont celles de l'agent, pas une approbation
humaine de chaque choix.

Le mandat initial limitait les écritures à ce document. L'autorisation ultérieure
du mode 2 étend les écritures à la réalisation locale de ces décisions et aux
validations disponibles. L'utilisateur a ensuite expressément autorisé l'installation
locale de cargo-llvm-cov et des outils LLVM, ainsi que les validations complémentaires
avant livraison. Aucun achat, lancement CI, upload de couverture Codecov, changement
administratif, commit, push, PR ou changement de Clint n'est effectué à ce stade.

**Résultat : réalisation et validations locales réussies, y compris un rapport
LCOV réel ; fonctionnement distant Codecov non vérifié.** Les résultats successifs
et limitations figurent en fin de document. Ne pas déclarer le pilote distant
fonctionnel avant ses preuves d'acceptation GitHub/Codecov.

## État constaté et invariants

Inspection du checkout au commit `6d0137ebfcec7a31ae2621d3e8852df10c62c38f`,
initialement sans modifications locales :

- [Cargo.toml](../../Cargo.toml) : un exécutable Rust 2024, MSRV 1.99.1,
  dépendances verrouillées ; profil de test optimisé à `opt-level = 1`.
- [rust-toolchain.toml](../../rust-toolchain.toml) : version locale 1.99.1.
- [CI](../../.github/workflows/ci.yml) : `push` sur `main` et PR vers `main` ;
  qualité ignorée sur les Draft PR ; tests Ubuntu et macOS après qualité,
  via `cargo test --locked --all-targets`. La CI utilise `stable` et vérifie
  séparément la MSRV.
- Le job `auto-merge` dépend actuellement seulement de `test` et appelle
  `gh pr merge --squash --delete-branch` pour une PR prête. Ajouter une couverture
  parallèle sans adapter cette dépendance permettrait une fusion avant sa fin.
- Aucun générateur de couverture ou fichier Codecov trouvé dans le checkout.
  L'installation de l'application GitHub ne produit pas elle-même de rapport Rust.
- [Tests CLI](../../tests/cli.rs) et [lifecycle](../../tests/lifecycle.rs) lancent
  le vrai binaire via `CARGO_BIN_EXE_fluidise-relay`, parfois après copie, via un
  hook ou un shell. Les commandes observées héritent de l'environnement ; ne pas
  casser cet héritage pendant la collecte des profils.
- Des tests vérifient les délais, signaux et processus descendants. Le test
  `native_interruption_is_preserved_after_child_cleanup` termine Relay par signal ;
  [main](../../src/main.rs) réémet le signal avec `emulate_default_handler`.
  Une sortie interrompue peut ne pas écrire son profil LLVM : couverture partielle
  de ces chemins possible, sans que le test fonctionnel ait échoué.
- Le test nécessitant `FLUIDISE_CLINT_BIN` est explicitement ignoré. Le pilote ne
  téléchargera pas Clint et ne prétendra pas couvrir cette intégration externe.

Préserver les tests natifs des deux OS, leurs assertions de sécurité, la MSRV,
les restrictions sur les Draft PR et les permissions minimales. La couverture
n'est ni une preuve de correction, ni une mesure de performance ou de portabilité.

## Décisions

### D1 — Produire un rapport LLVM de lignes sur Linux

Retenir `cargo-llvm-cov`, Rust `stable` comme la CI, avec le composant LLVM
correspondant (`llvm-tools-preview`) et un rapport LCOV unique sous `target/coverage/`.
Commande candidate pour la future réalisation, **non exécutée** :

```sh
cargo llvm-cov --locked --all-targets --lcov --output-path target/coverage/lcov.info
```

Créer préalablement le répertoire de sortie. Conserver le comportement des tests
ignorés par défaut. Vérifier que les entrées source retenues correspondent à
`src/`, sans tests ni dépendances ; ne pas masquer de modules de production pour
améliorer le taux. Ajuster le filtrage seulement si le rapport réel le nécessite.

La documentation indique que ce format fournit la couverture de lignes dans
Codecov. Pas de promesse de couverture de branches. Pas de nightly, de flags par
module, de matrice de couverture ou de changement du profil de test initialement.

Alternatives : statu quo insuffisant pour obtenir des mesures ; tarpaulin ajoute
un autre mécanisme sans besoin identifié ; collecte sur les deux OS plus coûteuse
et plus complexe à agréger pour un premier pilote. Conserver les tests natifs et
ajouter une exécution instrumentée Linux coûte une suite supplémentaire, mais
isole les effets de l'instrumentation et rend le retour arrière simple.

**Contre-épreuve requise :** observer des compteurs non nuls dans les chemins du
binaire exercés par les tests CLI et lifecycle, pas seulement dans le harnais de
tests. Comparer les résultats avec les tests natifs. Ne pas allonger les délais,
retirer des tests ou modifier le comportement des signaux pour obtenir un taux.

### D2 — Étendre la CI existante, séparer collecte et authentification

Ajouter un job de collecte Linux après `quality`, avec les mêmes événements et
la même exclusion des Draft PR que les tests existants. Permissions :
`contents: read`, sans secret ni `id-token: write`. Exécuter la suite instrumentée,
vérifier un rapport non vide et conserver ce rapport comme artefact GitHub du run.
Ne pas partager de cache de compilation instrumentée avec les tests ordinaires.

Ajouter un job d'envoi consommant uniquement l'artefact de ce run. Lui seul reçoit
`id-token: write` et `contents: read`. Checkout nécessaire à l'action Codecov,
avec `persist-credentials: false` ; ne lancer aucun script, test, build ou binaire
du dépôt dans ce job. Le rapport reste une entrée non fiable, pas du code à exécuter.

Un job unique serait plus court, mais donnerait l'accès OIDC aux tests et à leurs
sous-processus. La séparation ajoute un transfert d'artefact justifié par cette
frontière de confiance. Pas de nouveau workflow `pull_request_target` ou
`workflow_run` privilégié. Ne pas réutiliser le workflow de triage pour la collecte.

### D3 — OIDC, sans jeton Codecov durable

Pour les `push` sur `main` et les PR internes, retenir l'action officielle Codecov
avec `use_oidc: true`, sans `CODECOV_TOKEN`. Utiliser uniquement le rapport attendu
(`files` explicite, `disable_search: true`) et conserver la vérification d'intégrité
du CLI. L'authentification OIDC est documentée ; son acceptation effective pour
Fluidise/relay reste à démontrer.

Au stade réalisation, vérifier les versions publiées et épingler les actions par
SHA complet ainsi que la version de `cargo-llvm-cov`. Les sources Context7 ne sont
pas homogènes : l'exemple cargo-llvm-cov cite l'action v5, tandis que la documentation
de l'action sur `main` cite v7. Ne pas copier un tag ou inventer un SHA depuis ces
exemples. Vérifier aussi la version du CLI téléchargé par l'action : épingler
l'action seule ne fige pas nécessairement ce téléchargement.

Pour les PR de forks : conserver collecte et artefact sans privilège, mais ne pas
exécuter le job d'envoi pendant le pilote. Afficher explicitement cet envoi comme
non applicable, pas comme couverture Codecov réussie. Le support tokenless des
forks pourra être repris après vérification du caractère public du dépôt et des
règles Codecov, sans modification globale de l'organisation.

Alternatives rejetées initialement : secret durable inutile si OIDC fonctionne ;
tokenless généralisé dépendant de la visibilité et des paramètres administrateur
non vérifiés. Si OIDC est indisponible, bloquer l'envoi et conserver l'artefact ;
aucun repli silencieux sur un secret ou une offre payante.

### D4 — Mesurer avant de fixer des seuils bloquants

Configurer les statuts `project` et `patch` en `informational: true`, avec cible
relative `auto`, sans taux minimal absolu inventé. Activer un commentaire de PR
actualisé plutôt qu'une succession de commentaires ; privilégier les changements
de couverture, sans exiger un rapport de base pour le premier affichage.

Un seuil de 80 %, 90 % ou 100 % serait arbitraire sans mesure initiale fiable.
Les seuils bloquants et modifications de protection de branche sont donc exclus
du pilote. Le premier rapport `main` établira la base ; une PR antérieure à cette
base ne prouve pas une comparaison correcte. Les noms de fichiers doivent être
résolus sans correctif de chemins spéculatif.

Une erreur de test, de collecte ou un rapport vide doit échouer dans le job de
collecte. Pour l'envoi seulement, utiliser `fail_ci_if_error: true` et un échec
toléré à l'étape avec avertissement explicite dans le résumé du job : panne Codecov
visible mais non bloquante pendant le pilote. Ne pas tolérer toutes les erreurs
du job, et ne pas qualifier cet état de pilote validé.

Faire attendre `auto-merge` sur la collecte **et** le job d'envoi en plus des tests,
en acceptant explicitement le cas où l'envoi est ignoré pour un fork. Cela empêche
une course avec la collecte, mais n'attend pas le traitement asynchrone de Codecov
et ne crée pas de gate sur le pourcentage. Préserver l'échec des tests/qualité et
l'exclusion des Draft PR dans toute condition adaptée. Ne pas modifier le mécanisme
de fusion au-delà de ces dépendances nécessaires.

### D5 — Portage sur Clint seulement après preuve Relay

Le portage n'est pas autorisé dans cette phase. Réutiliser ensuite le principe,
pas des identifiants Relay ni un workflow copié aveuglément. Réinspecter les tests,
les événements, la matrice, la politique de fusion et les paramètres du service
de Clint. Pas de workflow partagé inter-dépôts avant un besoin réel démontré.

## Prérequis bloqués et branches exclues

- **État distant initialement non accessible :** `gh repo view Fluidise/relay --json
  visibility,defaultBranchRef` a échoué : `error connecting to api.github.com`.
  Le connecteur GitHub a ensuite permis de vérifier que Relay est public, avec
  `main` comme branche par défaut. Installation effective de l'application,
  protections et configuration du compte Codecov restent non vérifiées. La
  déclaration utilisateur ne vaut pas preuve de réception de rapports.
- **Offre gratuite et droits :** aucun achat autorisé. Vérifier dans le compte que
  Relay est accessible et que la fonctionnalité souhaitée est disponible sans
  paiement. Ne pas conclure que le dépôt est public à partir de son URL.
- **Versions exactes et runtime :** releases et SHA vérifiés lors du mode 2
  (voir ci-dessous). Compatibilité effective Rust/LLVM et authentification OIDC
  restent à démontrer. La phase initiale de documentation n'a installé ni exécuté
  d'outil de couverture ; le mode 2 n'a pas installé d'outil local non disponible.
- **Exclusions :** Clint, Test Analytics/JUnit, sélection automatique de tests,
  bundle analysis, badge README, refonte CI, seuil imposé aux fusions, administration
  globale Codecov/GitHub, releases et modifications de code métier.

Ces points ne bloquent pas les décisions locales ; ils bloquent l'affirmation
que l'intégration fonctionne et, selon le point, sa réalisation ou son envoi.

## Preuves d'acceptation pour une future phase de réalisation

1. Vérifier l'accès Relay, l'offre sans coût et les versions épinglées. Valider
   séparément la structure GitHub Actions et le schéma Codecov. Toute validation
   distante ou exécution CI nécessite son autorisation propre.
2. Conserver les succès des tests natifs Ubuntu/macOS et la vérification MSRV.
   Obtenir un LCOV non vide, des fichiers `src/` reconnus et des compteurs du vrai
   binaire issus des tests d'intégration ; noter les limitations liées aux signaux.
3. Contrôler l'échec explicite si un test échoue ou si le rapport manque ; contrôler
   l'avertissement non bloquant pour une erreur d'envoi. Aucune donnée sensible
   dans le rapport ou les logs, aucun secret ajouté.
4. Sur un run autorisé de `main`, observer le rapport Codecov associé au bon commit.
   Une réussite de l'étape d'envoi seule ne suffit pas.
5. Sur une PR interne autorisée après cette base, observer le bon commit de PR,
   le diff de couverture, les statuts informatifs et le commentaire actualisé.
   Vérifier que l'auto-merge ne précède pas la fin des jobs ajoutés. L'essai d'une
   PR prête peut fusionner automatiquement : ne pas le déclencher comme simple test
   sans autorisation de cette conséquence.
6. Vérifier qu'une Draft PR ne collecte pas et qu'une PR de fork ne reçoit ni
   secret ni droit OIDC pour la collecte, avec envoi ignoré conformément au pilote.
7. Noter durée et coût de collecte supplémentaires. Décider alors si l'expérience
   Relay justifie le portage sur Clint. Aucun pourcentage actuel n'est connu.

Retour arrière futur : retirer les deux jobs, restaurer les dépendances originales
de l'auto-merge et supprimer la configuration Codecov. Aucun secret durable ni
changement de politique d'organisation à annuler avec le choix retenu.

## Sources et validation de cette phase

Sources officielles consultées via Context7 le 2026-10-25 :

- [cargo-llvm-cov : usage, LCOV et intégration CI](https://github.com/taiki-e/cargo-llvm-cov/blob/main/README.md)
  et [options de test](https://github.com/taiki-e/cargo-llvm-cov/blob/main/docs/cargo-llvm-cov-test.txt).
- [Action Codecov : prérequis, OIDC et paramètres](https://github.com/codecov/codecov-action/blob/main/README.md),
  [implémentation de l'action](https://github.com/codecov/codecov-action/blob/main/action.yml).
- [Codecov : statuts informatifs et commentaires](https://docs.codecov.com/docs/quick-start),
  [règles des jetons et uploads tokenless](https://docs.codecov.com/docs/codecov-tokens).
- [GitHub : syntaxe et permissions des workflows](https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions).

La documentation récupérée sur `main` est évolutive, pas une preuve d'une version
installée ou publiée. Les invariants locaux viennent des fichiers liés ci-dessus.
La vérification de la phase initiale portait sur le document et la restriction des
modifications à celui-ci. Aucun test Rust, calcul de couverture, upload, appel
au validateur distant ou run CI n'a été effectué ; ils ne sont pas nécessaires
pour une modification documentaire et ne sont pas présentés comme réussis.

## Réalisation locale — mode 2

### Changements

- [CI](../../.github/workflows/ci.yml) : ajout de `coverage` après `quality`, sans
  cache instrumenté ni droit OIDC ; installation de cargo-llvm-cov 0.9.1 et du
  composant LLVM sur le runner uniquement ; tests instrumentés, contrôle du fichier
  non vide, de la présence de `src/main.rs` et d'au moins un compteur de ligne
  positif. Les exclusions natives de cargo-llvm-cov écartent notamment les fichiers
  des répertoires `tests`, `examples`, `benches` et les dépendances ; aucun filtrage
  supplémentaire spéculatif n'est ajouté.
- Conservation du rapport dans l'artefact `coverage-linux`, rétention de 7 jours,
  pour diagnostic et transfert au job d'envoi. Le contrôle des compteurs est une
  garde minimale, pas la preuve que les seuls tests d'intégration couvrent le binaire.
- Job `codecov` isolé, sans exécution de code du dépôt, envoi OIDC du fichier
  explicite uniquement, échec de l'étape d'envoi toléré avec avertissement et résumé.
  Aucun secret durable ajouté ; collecte maintenue pour les forks, envoi ignoré.
- Dépendances et garde d'`auto-merge` adaptées : tests et collecte doivent réussir,
  l'envoi doit finir avec succès ou être ignoré pour un fork. Une erreur technique
  hors de l'étape d'envoi reste bloquante. Les Draft PR et runs annulés sont exclus.
- [codecov.yml](../../codecov.yml) : statuts `project`/`patch` informatifs,
  `target: auto`, commentaire actualisé lorsque la couverture change, sans
  obligation de base pour le premier rapport.
- Aucun changement de Rust, des tests métier, de leurs délais ou de Clint.

### Versions vérifiées par le connecteur GitHub

Les endpoints officiels de releases, commits de tags et manifestes ont été lus :

- [codecov-action v7.1.1](https://github.com/codecov/codecov-action/releases/tag/v7.1.1),
  SHA `303a32d7a59b442fa8d48b6a1cc6825c09c847a5` ; paramètres OIDC et version du
  CLI vérifiés dans son `action.yml` à ce SHA.
- [codecov-cli v11.3.1](https://github.com/codecov/codecov-cli/releases/tag/v11.3.1),
  version explicite passée à l'action ; vérification d'intégrité non désactivée.
- [upload-artifact v7.0.2](https://github.com/actions/upload-artifact/releases/tag/v7.0.2),
  SHA `cf430e030ddbb5b0abf93d22962f4752f3646cd9`.
- [download-artifact v8.0.2](https://github.com/actions/download-artifact/releases/tag/v8.0.2),
  SHA `9000827ccba6bdab643e8b6fd33ac0654aef8333` ; sans identifiant de run ni token
  explicite, le téléchargement reste celui du run courant. Les manifestes des deux
  actions utilisent Node 24, fourni par les runners GitHub hébergés actuels.
- [cargo-llvm-cov v0.9.1](https://github.com/taiki-e/cargo-llvm-cov/releases/tag/v0.9.1),
  MSRV 1.87 dans son manifeste ; installation CI depuis Cargo, version et lock
  explicites. Cela ne prouve pas l'exécution sur la future version `stable` du runner.

Les actions préexistantes restent inchangées. Context7 a confirmé les exclusions
par défaut de cargo-llvm-cov, les statuts informatifs et les options de commentaire.

### Validations réellement effectuées

- `cargo fmt --all -- --check` : succès.
- `cargo clippy --offline --locked --all-targets -- -D warnings` : succès.
- `cargo test --offline --locked --all-targets` sur macOS : **83 tests réussis,
  1 échec, 1 ignoré**. Les 17 tests unitaires et 5 tests CLI réussissent ; lifecycle
  compte 61 succès, 1 échec et 1 ignoré. L'échec est
  `timeout_forces_same_group_descendant_after_parent_and_pipes_exit`, à
  `tests/lifecycle.rs:1043` : lancement de `ps` refusé avec
  `PermissionDenied` / `Operation not permitted` dans le sandbox. Aucun changement
  de ce test ni relâchement de sa vérification ; succès de la suite complète non acquis.
- YAML des deux fichiers analysé avec Ruby/Psych 3.1.0 disponible localement.
  Contrôles structurels : permissions, dépendances, actions épinglées, absence de
  token explicite, tolérance d'échec limitée à l'étape d'envoi ; tous réussis.
- Syntaxe des blocs shell vérifiée avec `bash -n` : succès.
- Garde effective d'auto-merge évaluée localement sur 1 024 combinaisons : événement,
  Draft, fork, annulation et états des trois dépendances. Résultats conformes aux
  attentes, notamment refus après échec/annulation/skip des tests ou de la collecte,
  et acceptation du skip d'envoi pour un fork seulement. Simulation booléenne,
  pas exécution du moteur d'expressions ou de l'ordonnanceur GitHub.
- Partie de validation LCOV exécutée avec six fixtures : fichier absent, vide,
  non-LCOV, tests seuls, compteurs nuls refusés ; fichier source avec compteur
  positif accepté. Aucun appel Cargo ni upload dans ces fixtures.
- `git diff --check` : succès. Liens documentaires locaux vérifiés.

### Limites constatées avant l'autorisation complémentaire

`cargo-llvm-cov`, `llvm-tools-preview` et `actionlint` ne sont pas installés
localement ; aucune installation effectuée. Donc pas de LCOV réel, pas de preuve
des compteurs des sous-processus, ni de mesure de taux/durée instrumentée. Analyse
YAML et fixtures ne remplacent ni `actionlint`, ni le validateur de schéma Codecov,
ni un run Linux réel. Aucun appel au validateur distant n'a été effectué.

La lecture GitHub fonctionne via le connecteur, mais pas via `gh` dans ce sandbox.
L'offre effective du compte, l'installation Codecov et OIDC ne sont toujours pas
vérifiées. Aucun run CI, upload, rapport de base `main`, commentaire de PR ou
traitement asynchrone Codecov observé. La politique de fusion reste automatique :
une future mise en état ready doit être autorisée en connaissance de cet effet.

La configuration locale est prête pour validation complémentaire, pas pour une
déclaration de pilote réussi. Les preuves restantes sont celles des critères
d'acceptation ci-dessus ; le portage sur Clint reste différé.

## Validation complémentaire autorisée avant livraison

Cette section actualise les résultats locaux précédents, sans effacer l'échec
initial dans le sandbox. L'utilisateur a répondu « Oui vas-y » à la proposition
d'installer cargo-llvm-cov et llvm-tools-preview, puis de valider avant livraison.

- Installation locale de **cargo-llvm-cov 0.9.1**, avec `--locked`, et du composant
  `llvm-tools-preview` pour Rust **1.99.1**, réussie. Aucun changement du manifeste,
  du lockfile du dépôt ou de la toolchain configurée. La toolchain locale nommée
  `stable` est ancienne (1.94.1) : les validations locales utilisent explicitement
  la 1.99.1 du projet ; la CI installe une `stable` à jour sur son runner.
- `cargo fmt --all -- --check` et
  `cargo clippy --offline --locked --all-targets -- -D warnings` : succès.
- `cargo test --offline --locked --all-targets`, hors sandbox : **84 succès,
  aucun échec, 1 test ignoré** (Clint externe). Le test lançant `ps` réussit sans
  changement de code, d'assertion ou de délai. L'échec antérieur était bien lié
  à la restriction d'exécution observée.
- `cargo llvm-cov --offline --locked --all-targets --lcov --output-path
  target/coverage/lcov.info`, hors sandbox : **84 succès, aucun échec, 1 ignoré**.
  Durées rapportées des suites instrumentées : 95,48 s pour les 17 tests unitaires,
  0,31 s pour les 5 tests CLI, 16,91 s pour lifecycle (62 succès, 1 ignoré).
- Rapport réel : **126 618 octets**, six fichiers `src/` seulement, tous avec des
  lignes exécutées. Somme LCOV : **2 555 / 3 176 lignes, soit 80,45 %** sur macOS
  ARM64/Rust 1.99.1. Il s'agit des lignes instrumentées dans les fichiers source,
  qui peuvent inclure les modules de tests intégrés à ces fichiers ; ce taux n'est
  pas une mesure exclusive du code de production ni la future base Linux Codecov.
- Le rapport contient notamment 454 appels de `main` et 451 de `parse` pour le
  binaire Relay, ainsi que des compteurs positifs dans les six modules. Cela
  confirme la remontée des profils du binaire lancé par les tests d'intégration,
  pas seulement du harnais de tests unitaires. Les limites des sorties par signal
  restent applicables.
- SHA-256 du LCOV local :
  `ddf355d35b22e1ed11cee53f47fe5dafb060c188d2ce5f8dd5e49995655af7fa`.
  Rapport et résumé JSON restent sous `target/coverage/`, ignorés par Git ; aucun
  chemin absolu local de ce rapport n'est ajouté à la configuration d'envoi.
- Validateur officiel `https://codecov.io/validate`, appelé uniquement avec
  `codecov.yml` : réponse **`Valid!`**, code de sortie 0. Aucun rapport de couverture
  envoyé ni réglage du service modifié par cette validation.
- Contrôles YAML/shell, 1 024 cas de garde de fusion et six fixtures LCOV rejoués :
  succès. `git diff --check` : succès. `actionlint` reste indisponible et n'a pas
  été installé ; ces contrôles ne remplacent pas un run GitHub Actions réel.
- Lectures GitHub hors sandbox possibles : dépôt public, branche par défaut `main`,
  SHA distant `6d0137ebfcec7a31ae2621d3e8852df10c62c38f`, identique à la base locale
  observée. Aucune PR ouverte lors de la lecture. La branche proposée
  `chore/codecov-pilot` n'existe pas à distance et aucune PR passée de ce nom n'est
  retournée par GitHub.

**Reste non vérifié :** runner Linux, envoi OIDC dans GitHub Actions, réception et
traitement du rapport par Codecov, comparaison à une base `main`, commentaire/statuts
de PR et comportement réel de la garde d'auto-merge. Aucune réussite de ces étapes
n'est déduite du rapport local ou du validateur YAML. La livraison envisagée est
une Draft PR uniquement, sans passage à ready, merge ou activation d'auto-merge.
