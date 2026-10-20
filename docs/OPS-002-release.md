# OPS-002 — Sortir une nouvelle version (`store.exe`)

Trois choses différentes, souvent mélangées :

| Mot | Ce que c'est |
|---|---|
| **commit** | une sauvegarde du code, sur une branche |
| **tag** (`v0.6.0`) | une étiquette sur un commit : « ceci est la version 0.6.0 » |
| **release** | la page publique GitHub avec le `.exe` à télécharger, attachée à un tag |

Le `.exe` n'est **jamais** construit à la main sur le PC du client : GitHub le construit
pour Windows, dans le nuage, à chaque tag poussé.

---

## La façon normale : un tag, et c'est tout

```bash
# 1. la version doit être la même partout
python3 scripts/bump_version.py patch      # 0.5.9 -> 0.5.10  (ou : 0.6.0)

# 2. on enregistre
git add -A
git commit -m "v0.5.10 — ce qui change, en une phrase"
git push origin arena/e4d9049c-desktop

# 3. on pose l'étiquette et on la pousse : c'est ça qui déclenche tout
git tag v0.5.10
git push origin v0.5.10
```

Ce qui se passe ensuite, tout seul, dans l'onglet **Actions** :

1. GitHub réveille une machine Windows ;
2. il installe Rust et compile `store.exe` (≈ 3 min la première fois, moins ensuite) ;
3. il lance les tests (l'argent doit toujours tomber juste, sinon il s'arrête) ;
4. il emballe un dossier `OneClick-0.5.10-windows-x64.zip` contenant
   `store.exe`, `OneClick.bat`, `A_LIRE.txt`, `LISEZMOI.md` ;
5. il crée la page **Releases** avec le zip et son empreinte SHA-256.

Le lien à donner au client :
`https://github.com/Nour-yahyaoui/desktop/releases/latest`

### Ce que le client fait

1. il télécharge le zip ;
2. il le dézippe où il veut (`C:\OneClick`) ;
3. il **double-clique sur `OneClick.bat`** — le navigateur s'ouvre sur la caisse ;
4. ses données vivent dans `data\`, à côté de `store.exe : copier ce dossier = une sauvegarde.

Rien à installer : pas de Node, pas de base de données, pas de .NET.

---

## Les garde-fous

- **Le tag doit correspondre à `Cargo.toml`.** Si la version du code est `0.5.9` et que tu
  pousses `v0.6.0`, le job s'arrête en une seconde avec un message clair : pas de release
  qui ment sur son numéro.
- **Les tests bloquent la publication.** Une erreur de calcul ne sort jamais chez un client.
- **Rien n'est jamais écrasé.** GitHub refuse un tag déjà poussé ; il faut bump la version.
- **La release n'est pas publiée si le build échoue.** Pas de zip vide.

## Refaire une release ratée

```bash
git tag -d v0.5.10                    # supprime le tag en local
git push origin :refs/tags/v0.5.10    # supprime le tag sur GitHub
```

Puis corrige, bump la version (par exemple `0.5.11`), et recommence. On ne réutilise pas
un numéro déjà publié : un client qui a téléchargé la `0.5.10` ne doit jamais se retrouver
avec un fichier différent sous le même nom.

## Construire le .exe sans attendre GitHub

Du moment que Rust est installé sur la machine :

```bash
./scripts/release.sh --dry-run    # tests + build + zip, sans publication
./scripts/release.sh              # idem, puis tag + push (GitHub publie)
```

Le zip sort dans `dist/` (dossier ignoré par git, jamais commité).

Sur une machine Windows, l'équivalent manuel :

```powershell
cargo test --all
cargo build --release --bin store     # -> target\release\store.exe
```

`store.exe --version` affiche la version gravée dans le binaire :
c'est le moyen le plus sûr de vérifier qu'un client a bien la bonne.

## Vérifier une version déjà sortie

```powershell
store.exe --version     # OneClick 0.5.10
store.exe --doctor      # base, schéma, points à vérifier
```

Si `--doctor` signale un problème, c'est la base du client qui est en cause, jamais
l'installation : les données sont dans `data\store.db`, un seul fichier à copier.

## Lire les couleurs de l'onglet Actions

Une poussée lance **deux** workflows différents :

| Workflow | Ce qu'il construit | Ce qu'il bloque |
|---|---|---|
| **ci** | la caisse, sur Linux **et** Windows, + les tests | une erreur de compilation ou un calcul faux |
| **release** | `store.exe` + le zip, uniquement sur un tag `vX.Y.Z` | la publication |

Dans **ci**, la moitié till (Linux + Windows) est bloquante ; le job
`control-plane` est **consultatif** : la moitié nuage est un service séparé et
optionnel, elle ne doit jamais empêcher la caisse de sortir. S'il est rouge, ça
ne concerne pas le `.exe` — mais ça reste à réparer, le log est là pour ça.

## En cas de pépin pendant le build

| Symptôme | Cause | Geste |
|---|---|---|
| `Le tag vX ne correspond pas à la version Y` | tag et `Cargo.toml` divergent | bump la version, nouveau tag |
| le job « Tests » échoue | un calcul ne tombe plus juste | corriger, **ne pas** publier |
| « release already exists » | le tag a déjà été publié | bump la version |
| pas de `.exe` dans la release | le job a échoué avant l'emballage | lire le log, corriger, re-tag |
