# OPS-003 — L'icône, le démarrage, et l'alerte « Windows a protégé votre PC »

## 1. L'icône (résolu)

`assets/oneclick.ico` contient les tailles 16, 24, 32, 48, 64, 128 et 256 px.
Il est **gravé dans `store.exe`** au moment de la compilation (`build.rs`, via
la crate `winresource`), et la fiche du fichier porte le nom du produit, la
version et l'éditeur.

- Explorateur, barre des tâches, raccourci sur le Bureau : même icône.
- Pour la modifier : `python3 scripts/make_icon.py` (le dessin est du code,
  rien n'est magique ni perdu), puis rebuild.
- Le zip contient aussi `OneClick.ico`, pour les raccourcis créés à la main.

## 2. L'alerte SmartScreen — pourquoi elle existe

Le message

> Windows protected your PC — Microsoft Defender SmartScreen prevented an
> unrecognized app from starting. App: store.exe Publisher: Unknown publisher

ne parle **pas** du contenu du programme. Il dit deux choses :

1. **« Éditeur inconnu »** : `store.exe` n'est pas **signé numériquement**. Pas
   de certificat = pas d'identité = Windows ne peut pas dire qui l'a fabriqué.
2. **« Application non reconnue »** : ce fichier n'a pas encore de réputation
   (SmartScreen compte les téléchargements et les avis).

**Aucune ligne de code ne peut supprimer ce message.** Il n'existe que trois
solutions réelles.

### Option A — Un certificat de signature (la seule solution propre)

| Type | Prix indicatif | Effet SmartScreen |
|---|---|---|
| **OV** (Organisation Validation, section 2 Comodo/Sectigo/DigiCert) | 250–400 € / an | Le message disparaît… **après** avoir construit une réputation (quelques semaines à quelques centaines de téléchargements) |
| **EV** (Extended Validation) | 400–700 € / an + token matériel | Réputation immédiate, message absent dès la première installation |
| **Azure Trusted Signing** (Microsoft) | ~9 $/mois | Signature gérée par Microsoft ; l'éligibilité dépend du pays et de l'ancienneté de l'entreprise — **à vérifier pour la Tunisie** avant d'y compter |

Ce qu'il faut fournir pour un OV/EV : registre de commerce, adresse vérifiable,
numéro de téléphone joignable (l'autorité appelle). Délai : 3 à 10 jours.

Une fois le certificat obtenu :

```powershell
# Une seule fois : importer le certificat (.pfx) dans le magasin utilisateur
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 `
  /f oneclick.pfx /p <mot-de-passe> store.exe
```

Puis dans `release.yml`, deux secrets GitHub (`WINDOWS_CERT_PFX` en base64,
`WINDOWS_CERT_PASSWORD`) et une étape `signtool` entre « Build » et
« Emballage ». La clé **ne doit jamais** être dans le dépôt.

### Option B — Le lanceur `OneClick.bat` (ce qui est déjà livré)

Windows ne met pas d'alerte sur un fichier `.bat` téléchargé dans un zip : il
l'applique à l'`.exe`. Le zip contient donc un `OneClick.bat` qui démarre
`store.exe` et ouvre le navigateur. Le client double-clique sur **le `.bat`**,
pas sur l'exe.

Ce n'est pas de la triche : c'est le chemin normal pour un petit éditeur. Le
`.bat` fait trois choses de plus que l'exe seul (le titre, le message
« ne fermez pas cette fenêtre », l'ouverture du navigateur).

> Limite honnête : dès qu'un client lance `store.exe` **directement**, l'alerte
> revient. Elle n'est donc évitée que sur le chemin qu'on lui indique.

### Option C — Vivre avec, et l'expliquer au client

Le message est en une phrase dans le `A_LIRE.txt` du zip :

> Au premier lancement, Windows demande confirmation : cliquez sur
> « Informations complémentaires » puis « Exécuter quand même ».
> Cela n'arrive qu'une fois.

C'est ce que font tous les petits logiciels non signés. Ce n'est pas tenable
pour un logiciel vendu à 200 magasins, ça l'est pour les dix premiers.

## 3. Recommandation

1. **Aujourd'hui** : option B + C (déjà en place). Rien à payer.
2. **À partir de ~10 clients payants** : certificat **OV** (~300 €/an) et
   signature automatique dans le workflow. C'est le prix de la confiance.
3. **Si l'entreprise est éligible** : Azure Trusted Signing (bien moins cher),
   à vérifier côté Tunisie.

## 4. À ne pas faire

- **Auto-signer** un certificat (`New-SelfSignedCertificate`) : Windows
  affichera *toujours* l'alerte, et en plus « certificat non approuvé ».
- **Signer avec un certificat volé ou prêté** : SmartScreen révoque la clé et
  tous les logiciels signés avec elle deviennent suspects.
- **Dire au client de désactiver SmartScreen** : c'est exactement ce qu'un
  logiciel malveillant demanderait. Ne jamais le demander.
