# Accueil des nouveaux contributeurs avec Welcome

## Mandat et résultat

- Date : 25 octobre 2026.
- Cible : `Fluidise/relay` uniquement, comme exemple et pilote avant toute éventuelle généralisation.
- Mode initial approuvé : décisions et documentation seulement. Extension ultérieure explicite « mode 2 go » : implémentation locale et validation de cette même conception.
- Autorisation : choisir les événements, les messages et leur langue selon les conventions du dépôt ; consigner les décisions dans ce document.
- Priorités : accueil utile, faible bruit, simplicité et réversibilité. Ces arbitrages sont délégués, pas des choix individuellement approuvés par l'utilisateur.
- Écritures permises après extension : ce document et `.github/config.yml` dans le checkout local. Aucune installation, modification des permissions GitHub, publication de commentaire, création de compte de test, modification de workflow, commit, push ou PR.
- Résultat : conception et configuration locale terminées ; validation statique locale effectuée. L'activation GitHub et le fonctionnement réel ne sont pas validés par cette intervention.

## État vérifié et limites des preuves

Le checkout inspecté (`6d0137ebfcec7a31ae2621d3e8852df10c62c38f`) ne contient pas de `.github/config.yml`. Les formulaires d'issues, le modèle de PR, `CONTRIBUTING.md` et `SECURITY.md` sont en anglais. Le workflow `pr-triage.yml` attribue et étiquette les PR ; il ne publie pas de message d'accueil. Les règles Git demandent des PR initialement Draft. Aucun changement du moteur Rust ni de ses hooks n'est nécessaire.

Les lectures GitHub de `Fluidise/relay/.github/config.yml` et de `Fluidise/.github/.github/config.yml` ont renvoyé 404. Cela ne prouve pas l'absence d'une configuration inaccessible ou d'une autre automatisation installée. Le lien fourni contient un identifiant d'installation et `setup_action=install` ; ce n'est pas une preuve que Welcome est actif ou autorisé sur Relay. Aucune inspection administrative de cette installation n'a été effectuée.

Le README officiel de Welcome documente trois clés dans `.github/config.yml` et des permissions **Issues: Read & Write** et **Pull requests: Read & Write**. Une valeur vide désactive le commentaire correspondant. Le paquet publié dans le dépôt source déclare la version `1.1.1`, Probot `9.3.0` et des dépendances GitHub vers les trois plugins. La version réellement déployée du service hébergé reste inconnue : le code consulté décrit le comportement attendu, pas une preuve de disponibilité du service.

Context7 ne référence pas Welcome dans les résultats obtenus. Une seconde résolution a permis de consulter la documentation officielle de Probot sur `context.config` et la convention `.github`. Cette documentation actuelle ne constitue pas une preuve du comportement exact de Probot 9.3.0 ; les clés et événements ci-dessous sont fondés sur les sources Welcome et ses plugins.

## Décisions et alternatives

### 1. Réutiliser l'application hébergée, sans nouveau workflow

Retenir Welcome pour le pilote demandé, sous réserve de vérifier son installation et son fonctionnement lors d'une étape ultérieure autorisée. Ne pas ajouter de runtime, dépendance Cargo, secret de dépôt ou workflow GitHub Actions.

Alternatives : conserver uniquement les modèles existants ne remplit pas l'objectif de commentaire d'accueil ; un workflow sur mesure pourrait filtrer les bots et contrôler la déduplication, mais crée une seconde automatisation à maintenir ; l'auto-hébergement ajoute une charge injustifiée pour ce pilote. Aucun remplacement silencieux si l'application hébergée ne fonctionne plus : constater l'échec et réévaluer alors.

La future installation devra cibler seulement Relay via la sélection de dépôts, et non tous les dépôts de Fluidise. Les permissions réellement affichées devront être comparées au README ; toute extension matérielle des accès devra être réévaluée avant activation.

### 2. Couvrir les trois étapes, sans confondre leurs historiques

Retenir les trois messages : première issue, première PR ouverte et première PR fusionnée. L’issue oriente vers le signalement privé des vulnérabilités, la PR vers le guide de contribution ; la fusion remercie sans répéter les consignes. À la demande ultérieure de l’utilisateur, raccourcir les textes et ajouter un emoji distinct par message : 👋 pour l’accueil, 🌱 pour la première PR et 🎉 pour la première fusion. Ne pas ajouter de GIF, mention de mainteneur, promesse de délai, label ou assignation.

Le terme « nouveau contributeur » désigne ici des historiques **par auteur et par dépôt**, séparés entre issues, PR ouvertes et PR fusionnées. Ce n'est ni la première contribution sur GitHub ni nécessairement une personne extérieure à l'organisation. Une même personne peut recevoir les trois messages sur trois étapes distinctes.

Comportements et contre-exemples relevés dans les plugins :

- `issues.opened` : l'auteur reçoit un accueil si la réponse de `issues.listForRepo(state: 'all', creator: ...)`, filtrée pour exclure les PR, contient exactement une issue. Une ancienne issue fermée compte aussi.
- `pull_request.opened` : même principe, avec filtrage des PR. Une PR Draft est donc éligible dès son ouverture ; `ready_for_review` n'est pas traité. Ne pas demander un passage immédiat en ready, qui déclencherait la CI et pourrait engager l'auto-merge existant.
- `pull_request.closed` : commentaire seulement si `merged` est vrai et si la recherche de PR fusionnées du même auteur dans le dépôt ne trouve aucune autre PR après exclusion du numéro courant. La première PR fusionnée peut ne pas être la première ouverte.
- Aucun filtre explicite des comptes bots ou des mainteneurs n'apparaît dans ces sources. Ne pas promettre un accueil réservé aux humains externes ; accepter cette limite pour le pilote sans ajouter une automatisation concurrente.
- Les listes issues/PR ne sont pas paginées explicitement ; une longue activité mêlant issues et PR peut fausser la détection. La recherche des merges dépend de l'index GitHub. Les sources ne montrent ni contrôle de doublon avant commentaire ni mécanisme de rattrapage. Ne pas garantir « exactement une fois », une détection exhaustive ou un accueil rétroactif.

Alternative écartée : première PR seulement. Elle réduit le bruit, mais exclut les personnes qui commencent par signaler un bug ou proposer une idée. Garder les messages courts rend les trois étapes utiles sans transformer l'accueil en checklist supplémentaire.

### 3. Messages en anglais, documentation de décision en français

L'anglais est cohérent avec les surfaces publiques du dépôt. Les messages français ou bilingues seraient moins homogènes ou plus longs. Employer des liens absolus vers les guides existants, sans réécrire leurs règles dans le bot.

L'accueil des issues rappelle le canal privé de sécurité sans demander de détails sensibles. Il arrive après l'ouverture d'une issue publique : il ne prévient pas une divulgation initiale et ne remplace pas `SECURITY.md`.

## Configuration retenue

Ce bloc est la spécification de [`.github/config.yml`](../../.github/config.yml), désormais créé localement après vérification de son absence. Aucune clé préexistante n'a été remplacée. La publication dans la branche par défaut reste nécessaire avant les essais ; une simple modification dans une branche de travail ne prouve pas son chargement par le service.

```yaml
# Welcome — https://github.com/behaviorbot/welcome
newIssueWelcomeComment: |
  👋 Thanks for opening your first issue! Great to have you here.
  Security concern? Please [report it privately](https://github.com/Fluidise/relay/blob/main/SECURITY.md).

newPRWelcomeComment: |
  🌱 Congrats on your first PR, and thanks for contributing!
  See our [contribution guide](https://github.com/Fluidise/relay/blob/main/CONTRIBUTING.md), and keep it in Draft until ready.

firstPRMergeComment: |
  🎉 Congrats on your first merged PR! Thanks for making fluidise-relay better.
```

## Validation future, dépendances et retour arrière

Ces critères définissent les preuves à recueillir si l'activation est autorisée. Le contrôle local du point 2 est effectué ci-dessous ; les contrôles GitHub et les essais réels restent non exécutés.

1. Vérifier dans les réglages GitHub l'identité de l'application Welcome, son état non suspendu, l'accès à Relay seulement, les permissions et l'absence d'un autre bot d'accueil concurrent. L'identifiant dans le lien utilisateur ne suffit pas.
2. Vérifier les trois valeurs YAML non vides, les liens et l'absence de modification des autres clés. Aucun workflow d'accueil additionnel ne doit être introduit.
3. Pour des contributions légitimes et consenties, observer un commentaire sur la première issue et sur la première PR Draft d'un auteur sans historique correspondant. Conserver les URLs, l'identité du bot et les messages réellement reçus comme preuves. Ne pas attendre de rattrapage sur les contributions antérieures à l'activation.
4. Vérifier qu'une deuxième issue et une deuxième PR du même auteur ne reçoivent pas cet accueil dans un historique simple ; rouvrir une contribution ou passer une PR en ready ne doit pas ajouter un accueil d'ouverture.
5. Lors d'une première fusion normale, observer le remerciement ; une fermeture sans fusion et une fusion ultérieure du même auteur ne doivent pas le déclencher dans le scénario simple. Ne jamais fusionner une PR artificiellement pour tester le bot, ni contourner les protections, la CI ou le circuit Draft existant. Sans fusion légitime, ce scénario reste non validé.
6. Un échec d'observation reste un échec ou une absence de preuve, même si le YAML est valide. Contrôler alors installation, configuration et santé du service avant toute généralisation. Les cas de pagination, indexation, concurrence et redélivrance restent des limites, pas des garanties validées par ces essais simples.

Retour arrière envisagé : vider les trois valeurs pour désactiver les messages selon le contrat documenté, en préservant les autres clés ; si une coupure de l'accès est nécessaire, retirer Relay des dépôts autorisés de l'application. Ces opérations nécessitent une autorisation ultérieure. Les commentaires déjà publiés ne disparaissent pas automatiquement.

## Implémentation et preuves locales — mode 2

- Périmètre : création de `.github/config.yml` avec les trois messages spécifiés, sans changement des workflows ni du code Rust. Le checkout ne contenait pas ce fichier avant l'écriture.
- Révision éditoriale demandée : messages raccourcis avec un emoji distinct chacun, un remerciement explicite et un accueil chaleureux ou des félicitations ; lien de sécurité, guide de contribution et consigne Draft conservés. Aucun événement ni accès supplémentaire.
- Analyse YAML : les trois clés attendues sont présentes, sans clé supplémentaire, et leurs valeurs sont des chaînes non vides ; aucun doublon de clé racine n'est présent. Vérification via Ruby/Psych installé, sans nouvelle dépendance (PyYAML n'est pas disponible dans le Python système).
- Conformité : égalité du YAML chargé depuis le fichier avec le bloc spécifié dans ce document ; les liens des messages ciblent uniquement `CONTRIBUTING.md` et `SECURITY.md` de Relay, présents localement ; la consigne Draft est conservée.
- Hygiène : vérification des espaces de fin de ligne sur les deux nouveaux fichiers et `git diff --check`. Aucun fichier suivi modifié ; seuls les deux livrables sont non suivis.
- Non exécuté : tests Rust, sans changement du programme ; essais d'événements GitHub, inspection administrative et activation, hors mandat local. Une analyse YAML réussie n'est pas une preuve runtime du bot.

## Disposition finale

- Résolu : solution proposée, cible pilote, trois événements, langue, messages, limites et protocole de validation.
- Inconnu avant activation : état réel de l'installation, version déployée, santé du service, permissions effectivement accordées et éventuelles automatisations concurrentes. Ces prérequis conditionnent l'activation, pas l'achèvement de cette conception documentaire.
- Exclu : déploiement sur les autres dépôts, auto-hébergement, modification des workflows de triage/CI, glossaire autonome et ADR. Aucun nouveau concept propre au domaine Relay ni arbitrage coûteux à inverser ne les justifie.
- Livrables : ce document mis à jour et `.github/config.yml` créé localement. Le périmètre local du mode 2 est terminé ; l'activation et la validation de bout en bout restent conditionnées à une autorisation de publication et aux prérequis GitHub ci-dessus. Aucun commit, push ou PR n'a été effectué ; aucune preuve runtime n'est revendiquée.

## Sources consultées

Sources lues le 25 octobre 2026 ; les liens vers les branches upstream sont évolutifs. Les identifiants ci-dessous sont les SHA de blobs renvoyés par GitHub, pas des versions du service hébergé.

- [Welcome — README](https://github.com/behaviorbot/welcome/blob/master/README.md), blob `7d8bc3e545dfd303448ae1eee111ff9f1c6f1526` : installation, permissions, clés et désactivation.
- [Welcome — package.json](https://github.com/behaviorbot/welcome/blob/master/package.json), blob `45b8ff523da99ad95a0b4f3d0aba377fd628725c` : composition des plugins et version déclarée.
- [new-issue-welcome — index.js](https://github.com/behaviorbot/new-issue-welcome/blob/master/index.js), blob `c69509f5748a2d7a82fc10d2e23f5b8e45c52822`.
- [new-pr-welcome — index.js](https://github.com/behaviorbot/new-pr-welcome/blob/master/index.js), blob `ea3b8b7b08ff275367be35c8cb97826269a966a7`.
- [first-pr-merge — index.js](https://github.com/behaviorbot/first-pr-merge/blob/master/index.js), blob `3bf2ef2b6c7577e13336dbbfbff5b8896c884d82`.
- [Probot — bonnes pratiques de configuration](https://github.com/probot/probot/blob/master/docs/best-practices.md), via Context7 `/probot/probot` : convention de configuration, avec la réserve de version explicitée plus haut.
- Sources locales : [contributions](../../CONTRIBUTING.md), [sécurité](../../SECURITY.md), [modèle de PR](../../.github/PULL_REQUEST_TEMPLATE.md), [formulaires d'issues](../../.github/ISSUE_TEMPLATE/), [triage](../../.github/workflows/pr-triage.yml), [règles Git](../git-workflow.md).
