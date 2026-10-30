# Distribution de fluidise-relay

Relay 0.1.0 utilise crates.io pour les sources et les GitHub Releases pour les
binaires Unix. Homebrew utilise le tap `Fluidise/homebrew-tap` sur macOS ARM64 et
Intel. Ces commandes nécessitent que la publication correspondante soit terminée.

```sh
cargo install fluidise-relay --version 0.1.0 --locked
brew install Fluidise/tap/fluidise-relay
```

Git est nécessaire à l'exécution. Cargo nécessite Rust 1.99.1 ou supérieur et un
éditeur de liens natif. Windows n'est pas pris en charge. Les binaires GNU sont
compilés sur Ubuntu 24.04 et ne promettent pas de fonctionner avec une glibc plus
ancienne. Les variantes musl sont disponibles séparément. Les binaires macOS sont
compilés sur macOS 15. Aucun installateur ne configure les hooks d'un dépôt :
les opérations inspect/prepare/install conservent leurs plans d'approbation.

Les archives comprennent le binaire, la licence MIT, `USAGE.md`, les notices
de tiers, l'inventaire `FILES.sha256` et `PROVENANCE.json`. Chaque archive a un
checksum `.sha256` ; `SHA256SUMS` rassemble les checksums de la release. La version
0.1.0 est non signée et non notariée Apple. Un checksum n'authentifie pas l'éditeur.

## Préparation sans publication

```sh
python3 -m unittest discover -s tests -p test_distribution.py -v
cargo publish --dry-run --locked --registry crates-io
python3 scripts/distribution.py prepare --target aarch64-apple-darwin --output /tmp/relay-candidate
python3 scripts/homebrew.py rehearse --distribution /tmp/relay-candidate
```

L'output doit être nouveau et hors du checkout. Le build nécessite le toolchain
déclaré, sa cible native et le composant `rust-docs`. Linux musl nécessite
`musl-gcc`. La répétition Homebrew n'est possible que sur macOS ; elle exécute les
méthodes install/test dans un préfixe temporaire, sans enregistrer de tap ni lier
le binaire dans le préfixe Homebrew. Elle ne remplace pas un audit Homebrew complet.

## Publication autorisée

1. Faire accepter le snapshot par une revue indépendante et obtenir la CI verte,
   incluant les six cibles de distribution. Merger selon la politique du dépôt.
2. Créer `v0.1.0` sur le commit validé de `main`. Le push déclenche `release.yml`.
   Un lancement manuel nécessite le même tag existant. Une release existante
   n'est pas écrasée ; vérifier son état avant toute reprise.
3. Le workflow construit des archives propres pour ce tag, exécute les smoke
   tests, vérifie leur identité et publie les assets et leurs SHA-256. Il ne
   pousse aucun commit de changelog sur `main`.
4. Relire la release, télécharger les six archives et vérifier `SHA256SUMS` ainsi
   que `scripts/distribution.py verify-release --distribution <directory>
   --tag v0.1.0 --source <commit complet>` depuis le checkout du tag.
5. Depuis ce checkout propre, relancer Cargo dry-run puis publier avec
   `cargo publish --locked --registry crates-io` via un identifiant existant
   autorisé. Ne jamais mettre de token dans la ligne de commande, les logs ou Git.
   Vérifier ensuite la version sur crates.io ; une version publiée est immuable.
6. Générer la formule avec `python3 scripts/homebrew.py release --distribution
   <directory> --tag v0.1.0 --source <commit complet> --output <nouveau fichier
   fluidise-relay.rb>`. La publier dans `Formula/fluidise-relay.rb` du tap sans
   toucher à CLINT, puis vérifier le contenu distant et les URLs des assets.

Ces commandes décrivent les interfaces, pas une autorisation permanente de
publication. En cas d'échec ou de réponse ambiguë, relire l'état distant avant
de reprendre. Ne jamais remplacer un asset existant ou déplacer un tag publié.

Le [document de décision](decisions/release-0.1.0.md) conserve le mandat,
les preuves et les étapes éventuellement bloquées.
