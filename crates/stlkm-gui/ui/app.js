const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const journal = document.getElementById("journal");
const grille = document.getElementById("grille");
const dedans = document.getElementById("dedans");
const compte = document.getElementById("compte");

/** Dernier verdict de mise à jour, par identifiant. */
let majParProjet = new Map();
let occupe = new Set();
/** Propriétaire ou simple utilisateur : décide de l'existence de l'Atelier. */
let moi = { owner: false, origin: "", account: "" };

function dire(message) {
  journal.textContent = message;
}

function erreur(e) {
  dire(typeof e === "string" ? e : (e?.message ?? String(e)));
}

// ── Store ─────────────────────────────────────────────────────────────────

async function chargerStore() {
  try {
    const projets = await invoke("projects");
    dessinerStore(projets);
    if (moi.owner) dessinerRepertoire(projets);
  } catch (e) {
    grille.innerHTML = "";
    grille.append(bloc("vide", `Répertoire illisible — ${e}`));
  }
}

function dessinerStore(projets) {
  grille.innerHTML = "";
  if (projets.length === 0) {
    grille.append(
      bloc("vide", `Aucune application publique sur le compte ${moi.account}.`),
    );
    return;
  }
  for (const projet of projets) grille.append(fiche(projet));
}

function fiche(p) {
  const carte = document.createElement("article");
  carte.className = "fiche" + (p.running ? " tourne" : "") + (p.hidden ? " retiree" : "");

  const tete = document.createElement("header");
  const emoji = document.createElement("div");
  emoji.className = "emoji";
  emoji.textContent = p.icon ?? (p.kind === "cli" ? "▶" : p.kind === "web" ? "◍" : "■");
  const titres = document.createElement("div");
  const titre = document.createElement("h2");
  titre.className = "titre";
  titre.textContent = p.name;
  const source = document.createElement("div");
  source.className = "details";
  source.textContent = p.source;
  titres.append(titre, source);
  tete.append(emoji, titres);

  const resume = document.createElement("p");
  resume.className = "resume";
  resume.textContent = p.summary;
  resume.title = p.summary;

  const meta = document.createElement("div");
  meta.className = "meta";
  if (p.hidden) meta.append(puce("retiré du répertoire", "neuf"));
  meta.append(puce(p.kind));
  if (p.running) meta.append(puce("en marche", "tourne"));
  else if (p.installed) meta.append(puce(p.version ?? "installé", "pose"));
  const maj = majParProjet.get(p.id);
  if (maj?.status === "dispo") meta.append(puce(`→ ${maj.latest ?? "nouveau"}`, "neuf"));
  for (const tag of p.tags) {
    if (tag !== p.kind) meta.append(puce(tag));
  }

  carte.append(tete, resume, meta, actions(p, maj));
  return carte;
}

function actions(p, maj) {
  const zone = document.createElement("div");
  zone.className = "actions";
  const travaille = occupe.has(p.id);

  if (!p.installed) {
    zone.append(bouton("Installer", "primaire", travaille, () => agir(p.id, "install")));
  } else if (p.running) {
    zone.append(bouton("Arrêter", "danger", false, () => arreter(p)));
  } else {
    zone.append(bouton("Lancer", "primaire", travaille, () => lancer(p)));
  }

  if (p.installed) {
    // On ne lance pas toujours un projet de la même façon : pouvoir aller voir
    // ses fichiers vaut autant qu'un bouton « Lancer ».
    zone.append(bouton("Ouvrir le dossier", "", false, () => ouvrirDossier(p)));
  }
  if (p.installed && maj?.status === "dispo") {
    zone.append(bouton("Mettre à jour", "", travaille, () => agir(p.id, "update_one")));
  }
  if (p.installed && !p.running) {
    zone.append(bouton("Désinstaller", "danger", travaille, () => agir(p.id, "uninstall")));
  }
  return zone;
}

async function ouvrirDossier(p) {
  try {
    const chemin = await invoke("reveal", { id: p.id });
    dire(chemin);
  } catch (e) {
    erreur(e);
  }
}

async function lancer(p) {
  try {
    dire(`Lancement de ${p.name}…`);
    const url = await invoke("launch", { id: p.id });
    dire(url ? `${p.name} tourne sur ${url}` : `${p.name} est lancé`);
  } catch (e) {
    erreur(e);
  }
  chargerStore();
}

async function arreter(p) {
  try {
    await invoke("stop", { id: p.id });
    dire(`${p.name} arrêté`);
  } catch (e) {
    erreur(e);
  }
  chargerStore();
}

async function agir(id, commande) {
  occupe.add(id);
  chargerStore();
  try {
    await invoke(commande, { id });
  } catch (e) {
    erreur(e);
  } finally {
    occupe.delete(id);
    chargerStore();
    rafraichirMaj();
  }
}

/// Vérifie les mises à jour — et au passage ce qui a été retiré du répertoire.
async function rafraichirMaj() {
  try {
    const checks = await invoke("updates");
    majParProjet = new Map(checks.map((c) => [c.id, c]));

    const dispo = checks.filter((c) => c.status === "dispo").length;
    const retires = checks.filter((c) => c.status === "retire").length;
    if (retires > 0) {
      dire(`${retires} application(s) retirée(s) du répertoire — tu peux les désinstaller`);
    } else if (dispo > 0) {
      dire(`${dispo} mise(s) à jour disponible(s)`);
    }

    dessinerStore(await invoke("projects"));
  } catch (e) {
    erreur(e);
  }
}

// ── Atelier : le contenu du répertoire ────────────────────────────────────

function dessinerRepertoire(projets) {
  dedans.innerHTML = "";

  // L'Atelier est le répertoire du PC. Ce qui n'a qu'un APK vit dans l'Atelier mobile :
  // inutile de faire défiler soixante dépôts pour ranger ce que les fichiers disent déjà.
  const surPc = projets.filter((p) => p.platforms.length === 0 || p.platforms.includes("windows"));
  const ailleurs = projets.length - surPc.length;
  compte.textContent =
    `${surPc.length} application(s) — dépôts publics de ${moi.account}` +
    (ailleurs > 0 ? ` · ${ailleurs} rangée(s) côté mobile` : "");

  if (surPc.length === 0) {
    dedans.append(bloc("vide", "Le répertoire est vide."));
    return;
  }

  for (const p of surPc) dedans.append(ligneRepertoire(p));
}

function ligneRepertoire(p) {
  const ligne = document.createElement("div");
  ligne.className = "trouve";

  const gauche = document.createElement("div");
  const nom = document.createElement("div");
  nom.className = "nom";
  nom.textContent = `${p.name}  ·  ${p.kind}`;
  const details = document.createElement("div");
  details.className = "details";
  details.textContent = `${p.id} — ${p.source}`;
  gauche.append(nom, details);

  if (p.from_file) gauche.append(bloc("details", "fiche corrigée à la main"));

  const droite = document.createElement("div");
  droite.className = "actions";
  if (p.hidden) {
    droite.append(bouton("Remettre", "", false, () => masquer(p, "unhide", "remis dans le répertoire")));
  } else {
    droite.append(
      bouton("Retirer du répertoire", "danger", false, () =>
        masquer(p, "hide", "retiré du répertoire"),
      ),
    );
  }

  ligne.append(gauche, droite);
  return ligne;
}

/** Remet une fiche rangée dans les deux hubs. */
async function ranger(p, plateformes, quoi) {
  try {
    await invoke("set_platforms", { id: p.id, platforms: plateformes });
    dire(`${p.name} ${quoi} — publie pour que ça vaille chez les autres`);
    chargerStore();
    chercherApplisMobiles();
  } catch (e) {
    erreur(e);
  }
}

async function masquer(p, commande, quoi) {
  try {
    await invoke(commande, { id: p.id });
    dire(`${p.name} ${quoi} — publie pour que ça vaille chez les autres`);
    chargerStore();
  } catch (e) {
    erreur(e);
  }
}

// ── Atelier mobile : ce que les releases contiennent ──────────────────────
//
// Une release avec un .apk donne une appli Android ; sans rien pour le PC, elle
// n'a rien à faire dans le répertoire du bureau. On ne le décrète pas, on le lit.

const dedansMobile = document.getElementById("dedans-mobile");
const compteMobile = document.getElementById("compte-mobile");
const chercherMobile = document.getElementById("chercher-mobile");

chercherMobile.addEventListener("click", chercherApplisMobiles);

async function chercherApplisMobiles() {
  chercherMobile.disabled = true;
  compteMobile.textContent = "Recherche… un appel par dépôt, sois patient.";
  dedansMobile.innerHTML = "";
  try {
    dessinerMobile(await invoke("scan_mobile"));
  } catch (e) {
    erreur(e);
    compteMobile.textContent = "La recherche a échoué.";
  } finally {
    chercherMobile.disabled = false;
  }
}

function dessinerMobile(applis) {
  dedansMobile.innerHTML = "";
  compteMobile.textContent = `${applis.length} appli(s) Android — dernière release de chaque dépôt`;

  if (applis.length === 0) {
    dedansMobile.append(
      bloc("vide", "Aucune release ne contient d'APK. Publies-en une, et elle apparaîtra ici."),
    );
    return;
  }

  for (const a of applis) {
    const ligne = document.createElement("div");
    ligne.className = "trouve";

    const gauche = document.createElement("div");
    const nom = document.createElement("div");
    nom.className = "nom";
    nom.textContent = `${a.name}  ·  ${a.tag}`;
    const details = document.createElement("div");
    details.className = "details";
    details.textContent = a.apk + (a.autres.length ? ` (+ ${a.autres.join(", ")})` : "");
    gauche.append(nom, details);
    gauche.append(
      bloc(
        "details",
        a.aussiPc
          ? "la release sert aussi le PC : la fiche reste dans les deux hubs"
          : "rien pour le PC dans cette release : à ranger côté mobile",
      ),
    );

    if (a.platforms.length > 0) gauche.append(bloc("details", `rangée : ${a.platforms.join(", ")}`));

    const droite = document.createElement("div");
    droite.className = "actions";
    droite.append(bouton("Ranger", "primaire", false, () => rangerMobile(a)));
    if (a.platforms.length > 0) {
      droite.append(
        bouton("Remettre partout", "", false, () =>
          ranger(a, [], "remis dans les deux hubs"),
        ),
      );
    }

    ligne.append(gauche, droite);
    dedansMobile.append(ligne);
  }

  const aRanger = applis.filter((a) => !a.aussiPc);
  if (aRanger.length > 1) {
    const tout = document.createElement("div");
    tout.className = "trouve";
    tout.append(
      bloc("details", `${aRanger.length} applis n'ont rien pour le PC.`),
      bouton("Tout ranger", "primaire", false, async () => {
        for (const a of applis) await rangerMobile(a, true);
        dire("Rangé — publie pour que ça vaille chez les autres");
        chargerStore();
      }),
    );
    dedansMobile.append(tout);
  }
}

async function rangerMobile(a, silencieux = false) {
  try {
    await invoke("file_mobile", { id: a.id, apk: a.apk, aussiPc: a.aussiPc });
    if (!silencieux) {
      dire(`${a.name} rangé — publie pour que ça vaille chez les autres`);
      chargerStore();
    }
  } catch (e) {
    erreur(e);
  }
}

// ── Le jeton GitHub ───────────────────────────────────────────────────────
//
// Il ne ressort jamais d'ici : on peut le remplacer ou l'effacer, pas le relire.
// Il vit dans le dossier de données du hub, à part de config.toml, pour qu'un
// fichier de réglages qu'on se passe n'emporte jamais un mot de passe avec lui.

const dJeton = document.getElementById("jeton");
const jetonEtat = document.getElementById("jeton-etat");
const jetonValeur = document.getElementById("jeton-valeur");

document.getElementById("jeton-ouvrir").addEventListener("click", async () => {
  jetonValeur.value = "";
  jetonEtat.textContent = "Lecture du quota…";
  dJeton.showModal();
  montrerJeton(await etatJeton());
});

document.getElementById("jeton-fermer").addEventListener("click", () => dJeton.close());

document.getElementById("jeton-garder").addEventListener("click", async () => {
  jetonEtat.textContent = "Vérification auprès de GitHub…";
  try {
    montrerJeton(await invoke("set_token", { valeur: jetonValeur.value }));
    jetonValeur.value = "";
    dire("Jeton enregistré.");
  } catch (e) {
    jetonEtat.textContent = typeof e === "string" ? e : String(e);
  }
});

async function etatJeton() {
  try {
    return await invoke("token_state");
  } catch (e) {
    erreur(e);
    return null;
  }
}

function montrerJeton(etat) {
  if (!etat) {
    jetonEtat.textContent = "GitHub n'a pas répondu.";
    return;
  }
  jetonEtat.textContent = etat.present
    ? `Un jeton est en place — ${etat.restant} demandes restantes sur ${etat.limite} par heure.`
    : `Aucun jeton — ${etat.restant} demandes restantes sur ${etat.limite} par heure.`;
}

// ── Publication ───────────────────────────────────────────────────────────

const dialogue = document.getElementById("confirmation");
const dTitre = document.getElementById("confirmation-titre");
const dCorps = document.getElementById("confirmation-corps");
const dMessage = document.getElementById("confirmation-message");
const dOui = document.getElementById("confirmation-oui");

async function preparerPublication() {
  let plan;
  try {
    plan = await invoke("publish_info");
  } catch (e) {
    erreur(e);
    return;
  }

  dCorps.innerHTML = "";
  dTitre.textContent = plan.remote ? `Publier sur ${plan.remote}` : "Publier (aucun remote)";

  if (plan.blocked) {
    dOui.disabled = true;
    dCorps.append(bloc("bloquant", "Des secrets traînent dans le répertoire :"));
    for (const w of plan.warnings) dCorps.append(bloc("diff", `ligne ${w.line} — ${w.reason}`));
  } else if (plan.empty) {
    dOui.disabled = true;
    dCorps.append(bloc("details", "Le répertoire n'a pas bougé."));
  } else {
    dOui.disabled = false;
    for (const c of plan.changes) dCorps.append(bloc("diff", c));
    if (!plan.remote) dCorps.append(bloc("details", "Aucun remote : le commit restera local."));
  }

  dMessage.value = "répertoire : mise à jour";
  dialogue.showModal();
}

dOui.addEventListener("click", async () => {
  dialogue.close();
  try {
    await invoke("publish_now", { message: dMessage.value });
    dire("Répertoire publié");
  } catch (e) {
    erreur(e);
  }
});

document.getElementById("confirmation-non").addEventListener("click", () => dialogue.close());

// ── Outils ────────────────────────────────────────────────────────────────

function bouton(texte, classe, desactive, action) {
  const b = document.createElement("button");
  b.textContent = texte;
  if (classe) b.className = classe;
  b.disabled = Boolean(desactive);
  b.addEventListener("click", action);
  return b;
}

function puce(texte, classe) {
  const s = document.createElement("span");
  s.className = "puce" + (classe ? ` ${classe}` : "");
  s.textContent = texte;
  return s;
}

function bloc(classe, texte) {
  const d = document.createElement("div");
  d.className = classe;
  d.textContent = texte;
  return d;
}

// ── Mise en route ─────────────────────────────────────────────────────────

for (const onglet of document.querySelectorAll(".onglet")) {
  onglet.addEventListener("click", () => {
    for (const autre of document.querySelectorAll(".onglet")) {
      autre.classList.toggle("actif", autre === onglet);
    }
    for (const vue of document.querySelectorAll(".vue")) {
      vue.classList.toggle("active", vue.id === onglet.dataset.vue);
    }
  });
}

document.getElementById("publier").addEventListener("click", preparerPublication);

document.getElementById("actualiser").addEventListener("click", async () => {
  dire("recherche des nouveautés…");
  try {
    const combien = await invoke("refresh");
    dire(`Répertoire à jour — ${combien} application(s)`);
  } catch (e) {
    erreur(e);
  }
  await chargerStore();
  rafraichirMaj();
});

listen("etape", (event) => dire(`${event.payload.id} — ${event.payload.message}`));

// ── Le hub lui-même ───────────────────────────────────────────────────────

const majHub = document.getElementById("maj-hub");

/// Regarde s'il existe une version plus récente de STLKM. Silencieux s'il n'y
/// en a pas : personne n'a envie qu'on lui parle pour ne rien dire.
async function verifierLeHub() {
  try {
    const version = await invoke("hub_check");
    if (!version) return;

    majHub.textContent = `Mettre à jour STLKM (${version})`;
    majHub.hidden = false;
    majHub.onclick = async () => {
      majHub.disabled = true;
      dire("téléchargement de la nouvelle version…");
      try {
        // Le hub redémarre tout seul une fois installé.
        await invoke("hub_install");
      } catch (e) {
        erreur(e);
        majHub.disabled = false;
      }
    };
  } catch (e) {
    // Pas de réseau, pas de release : ça ne doit pas gêner l'utilisation.
    console.warn(e);
  }
}

(async function demarrer() {
  // Savoir quelle version tourne sur quelle machine : indispensable dès qu'il
  // y a plus d'un ordinateur.
  try {
    document.getElementById("version").textContent = `v${await invoke("hub_version")}`;
  } catch (e) {
    console.warn(e);
  }

  try {
    moi = await invoke("mode");
  } catch (e) {
    erreur(e);
  }
  // L'Atelier n'existe que pour le propriétaire du répertoire.
  document.getElementById("onglet-atelier").hidden = !moi.owner;
  document.getElementById("onglet-mobile").hidden = !moi.owner;
  await chargerStore();
  rafraichirMaj();
  verifierLeHub();
})();
