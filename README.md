# STLKM — le hub

Un répertoire d'applications : tes projets d'un côté, un bouton « Installer »
de l'autre.

**Tu n'as aucune liste à tenir.** Le hub lit ton compte GitHub : tout dépôt
public que tu pousses apparaît dedans, chez toi comme chez les gens qui ont
installé le hub.

---

## Ouvrir le hub

**Double-clic sur `STLKM` sur ton Bureau** (l'icône orange à deux tiroirs).

Le fichier `STLKM.bat` du dossier fait la même chose, si tu préfères partir
d'ici.

## Ouvrir la console

**Double-clic sur `Console STLKM.bat`.** Une fenêtre noire s'ouvre, dans
laquelle la commande `stlkm` existe. Tape `stlkm` seul pour voir la liste.

La console fait exactement la même chose que la fenêtre. Elle sert surtout
quand tu veux aller vite, ou sur une machine sans écran.

---

## Ce que font les deux

| Ce que tu veux | Dans la fenêtre | Dans la console |
|---|---|---|
| Voir le répertoire | onglet **Store** | `stlkm list` |
| Installer une appli | bouton **Installer** | `stlkm install <id>` |
| La lancer | bouton **Lancer** | `stlkm run <id>` |
| L'arrêter | bouton **Arrêter** | `Ctrl-C` dans la console |
| Mettre à jour | bouton **Mettre à jour** | `stlkm update <id>` |
| Voir ce qui a pris du retard | bouton **Actualiser** | `stlkm update` |
| Voir où sont ses fichiers | bouton **Ouvrir le dossier** | `stlkm where <id>` (affiche) · `stlkm open <id>` (ouvre) |
| Désinstaller | bouton **Désinstaller** | `stlkm uninstall <id>` |

`<id>` est l'identifiant court affiché à gauche dans `stlkm list`
(`cleanfiles`, pas `CleanFiles`).

Un outil en ligne de commande s'ouvre dans sa propre fenêtre noire. Elle se
ferme toute seule quand le programme se termine, et ne reste ouverte que s'il
a planté — de quoi lire l'erreur. Tout le reste (téléchargements, `git`,
`npm ci`…) tourne sans rien afficher.

---

## Remplir le répertoire

**Tu pousses un dépôt public sur GitHub. C'est tout.** Il apparaît dans le hub
au prochain *Actualiser*, avec son nom, sa description et son langage.

Le hub devine aussi comment le lancer, en regardant ses fichiers une fois
installé : un `.py` → il ouvre un terminal dessus, un `package.json` avec un
script `dev` → il démarre le serveur et ouvre le navigateur, un `index.html`
seul → il sert le dossier, un `.exe` → il l'exécute.

Ce qui n'apparaît **pas** : les dépôts privés, les copies d'autres projets
(*forks*), les dépôts archivés, les dépôts vides, et ceux portant le sujet
GitHub `stlkm-ignore`.

## Retirer une application — l'Atelier

L'onglet **Atelier** n'apparaît que chez toi (voir « Qui voit quoi »). Il liste
tout ce que le répertoire contient, avec un bouton **Retirer du répertoire** sur
chaque ligne.

Retirer ne supprime rien : ça ajoute simplement l'identifiant à une liste, dans
`catalog.toml` :

```toml
hidden = ["vieux-truc", "essai"]
```

L'application disparaît alors du hub — chez toi, et chez tout le monde une fois
publié. Le bouton **Remettre** annule.

Un cas particulier utile : si tu retires une application que tu as encore
installée, elle **reste visible chez ceux qui l'ont**, marquée « retiré du
répertoire », avec son bouton Désinstaller. Personne ne se retrouve avec un
logiciel installé qui a disparu sans explication. La vérification se fait en
même temps que celle des mises à jour.

Le fichier `catalog.toml` sert aussi à **corriger** une fiche quand la
devinette se trompe : un vrai nom, une icône, un résumé, une commande de
lancement précise. Une fiche du fichier remplace celle devinée depuis GitHub.
Tout ça est facultatif.

## Publier tes corrections

Tant que tu n'as pas publié, tes corrections ne sont que chez toi.

- **Dans la fenêtre** : onglet Atelier → **Publier le répertoire**. Une fenêtre
  te montre ce qui partirait, tu confirmes ou tu annules.
- **Dans la console** : `stlkm publish`. La question est posée dans le terminal
  (`o` pour oui).

Deux refus automatiques, dans les deux cas :

- si quelque chose ressemble à un mot de passe ou à une clé dans le fichier,
  la publication s'arrête et te dit la ligne ;
- seul `catalog.toml` est envoyé, jamais un fichier voisin.

---

## Qui voit quoi

Le hub regarde s'il trouve **ton clone local** du dépôt catalogue
(`C:\Users\favou\Desktop\Code\stlkm-catalog`).

- **Il le trouve** → tu es propriétaire : l'Atelier existe, tu peux publier.
- **Il ne le trouve pas** → simple utilisateur : il télécharge le répertoire
  publié sur GitHub, l'affiche, installe et met à jour. Pas d'Atelier, aucune
  écriture possible.

La vraie serrure n'est pas cet interrupteur, c'est GitHub : publier demande le
droit d'écriture sur `Yurogin/stlkm-catalog`, que personne d'autre n'a.

Pour voir les réglages actuels : `stlkm config`.

---

## Une règle à retenir

**Un projet qui n'est pas sur GitHub n'est utile qu'à toi.** Le hub sait le
lancer sur ta machine, mais personne d'autre n'aurait rien à télécharger.
Pousse-le, et il entre au répertoire tout seul.

---

## Modifier le code

Le code est en Rust, mais tu n'as pas besoin d'y toucher pour utiliser le hub.
Si tu modifies quelque chose : **double-clic sur `Recompiler.bat`**. Il
recompile et remplace les programmes du dossier `bin`.

Organisation, si tu veux regarder :

```
crates/stlkm-core/   le moteur : découverte GitHub, installation, lancement
crates/stlkm-cli/    la console
crates/stlkm-gui/    la fenêtre (son interface est dans ui/, en HTML et CSS)
bin/                 les programmes prêts à lancer
```

La fenêtre et la console ne savent rien faire par elles-mêmes : elles appellent
toutes les deux le moteur. Une correction dans le moteur profite aux deux.
