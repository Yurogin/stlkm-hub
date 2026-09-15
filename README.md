# STLKM

Un répertoire d'applications : les projets publiés par
[Yurogin](https://github.com/Yurogin), installables et lançables depuis une
seule fenêtre.

## Installer

**[Télécharger STLKM pour Windows](https://github.com/Yurogin/stlkm-hub/releases/latest/download/STLKM-setup.exe)**

Double-clic sur le fichier téléchargé et c'est installé. Windows 10 et 11,
moins de 4 Mo.

## Ce que ça fait

La fenêtre affiche les applications disponibles, chacune avec ses boutons :

- **Installer** — récupère le code depuis GitHub et prépare ce qu'il faut,
  dépendances comprises ;
- **Lancer** — le hub sait tout seul s'il doit ouvrir un terminal, démarrer un
  serveur et ouvrir le navigateur, ou exécuter un programme ;
- **Ouvrir le dossier** — pour aller voir les fichiers d'une application ;
- **Mettre à jour** — quand une nouvelle version a été publiée ;
- **Désinstaller** — qui n'efface que ce que le hub avait téléchargé.

Rien n'est installé ailleurs que dans le dossier du hub, et rien ne démarre
avec Windows.

## Mises à jour

Le hub regarde à chaque ouverture s'il existe une version plus récente de
lui-même. Si oui, un bouton apparaît en haut de la fenêtre ; sinon il ne dit
rien. Ces mises à jour sont **signées** : une version qui ne l'a pas été avec
la bonne clé est refusée.

Les applications sont suivies séparément — le hub compare ce qui est installé
chez toi à ce qui est publié, et te signale ce qui a pris du retard.

## Ce qu'il faut sur la machine

- **Windows 10 ou 11.** Le programme fonctionne aussi sous Linux, mais aucune
  version n'y est publiée pour l'instant.
- **git**, pour récupérer les applications.
- Selon l'application lancée : `python`, `node` ou `php`. Le hub ne les
  installe pas à ta place, mais il te dit lequel manque.

## Vie privée

Le hub ne crée aucun compte et n'envoie rien nulle part. Il lit la liste
publique des dépôts, télécharge ce que tu lui demandes, et garde sur ta
machine la seule liste de ce que tu as installé.
