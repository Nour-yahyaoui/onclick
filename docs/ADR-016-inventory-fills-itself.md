# ADR-016 — L'inventaire se remplit tout seul

**Statut :** adopté (v1.0)
**Contexte :** la plupart des petits magasins de téléphone n'ont jamais saisi leur
stock, et ne le saisiront jamais. Les logiciels classiques exigent un catalogue
avant la première vente : ils sont donc abandonnés le premier jour.

## Décision

1. **On vend d'abord, on décrit ensuite.** Un code scanné inconnu n'est jamais
   une erreur : il ouvre deux questions (nom *facultatif*, prix de vente, prix
   d'achat *facultatif*), crée la fiche, et met l'article dans le panier.
2. **Un produit sans nom est un produit valide.** Partout où il est affiché, il
   devient « Code 123456 ». Aucun écran ne réclame le nom.
3. **Un produit né d'un scan ne compte pas son stock.** `tracks_stock = 0` :
   la caisse n'affiche ni « 0 en stock » ni « rupture », parce que le magasin
   n'a jamais prétendu avoir un stock. Vendre reste possible, sans limite.
4. **La fiche se complète au calme.** La page produit montre ce que l'article a
   réellement fait (quantité vendue, total, dernière vente) : c'est avec ces
   chiffres, pas avec une théorie, que le patron fixe son prix et son prix
   d'achat.
5. **On ne bloque jamais pour une donnée manquante.** Ni nom, ni prix d'achat,
   ni stock : la vente passe. Le manque est signalé (« prix à définir »), jamais
   interdit.

## Conséquences

- Un magasin peut ouvrir la caisse **vide** et travailler immédiatement.
- Le stock affiché n'est fiable que pour les produits que le magasin compte
  vraiment ; l'interface le dit au lieu de mentir (`—` au lieu de `0`).
- La marge n'est connue que si un prix d'achat a été saisi un jour. C'est
  assumé : *mieux vaut une marge approximative qu'un magasin qui n'ouvre pas.*
- `sale_lines.unit_cost` fige le coût au moment de la vente : compléter un prix
  d'achat plus tard ne réécrit pas le passé.

## Charges fixes (même logique)

Le loyer se paie au mois, l'assurance au trimestre, la patente à l'année. On
stocke **le montant réellement payé et sa période**, jamais un montant
« mensualisé » : le calcul se fait à l'affichage (`montant ÷ 1, 3 ou 12`). Une
charge arrêtée n'est jamais supprimée : elle porte `stopped_day`, pour qu'un
mois passé reste explicable.
