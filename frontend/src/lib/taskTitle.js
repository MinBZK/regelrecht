/**
 * taskTitle - de regel die een taak in de lijst toont.
 *
 * Bewust NIET `task.title` van de server. De worker stelt die string samen met
 * alleen het law_id in de hand (worker.rs:1495: `format!("Verrijking
 * beoordelen: {}", payload.law_id)`), dus daar staat onvermijdelijk
 * "wet_op_de_zorgtoeslag" in plaats van "Wet op de zorgtoeslag" - de
 * weergavenamen leven in het corpus, dat alleen de frontend bevraagt. Wie de
 * naam kent moet de zin schrijven, dus doen we het hier.
 *
 * Toon: gebiedende wijs voor wat jij moet doen ("Beoordeel …"), mededelend voor
 * wat er buiten je om gebeurde ("… is mislukt") of waar je op wacht - een
 * mislukking is geen opdracht.
 *
 * `task.title` blijft de terugval voor een payload-vorm die we niet kennen; dan
 * is de kale servertekst nog altijd beter dan een lege regel.
 */
import { taskContext, WERKDOCUMENTEN, WET } from './taskCategories.js';

// Werkdocumenten dragen een pad (`mvt/concept.md`); de bestandsnaam is het
// handvat dat de gebruiker herkent, net als in de werkdocumentenlijst.
function fileName(path, fallback = 'werkdocument') {
  return path?.split('/').pop() || fallback;
}

/**
 * @param {object} task
 * @param {(lawId: string) => string} lawName Weergavenaam voor een law_id
 *   (useCorpusLaws' displayName; valt zelf terug op humanizeLawId).
 */
export function taskTitle(task, lawName = (id) => id) {
  const failed = task?.task_type === 'job_failed';
  const context = taskContext(task);

  if (context === WET) {
    const naam = lawName(task.payload.law_id);
    return failed ? `Verrijking van ${naam} is mislukt` : `Beoordeel verrijking van ${naam}`;
  }
  if (context === WERKDOCUMENTEN) {
    const naam = fileName(task.payload.target_path);
    return failed ? `Conversie van ${naam} is mislukt` : `Beoordeel werkdocument ${naam}`;
  }
  return task?.title ?? '';
}

/**
 * De regel voor een lopende job. Spiegelt de categorie "Wachten op": dit is
 * niets om te doen, alleen om te weten.
 *
 * Een document_convert-job draagt een synthetische `doc:`-sleutel als law_id,
 * dus die leest zijn naam uit target_path.
 */
export function runningTitle(job, lawName = (id) => id) {
  if (job?.job_type === 'document_convert') {
    return `Wachten op conversie van ${fileName(job.target_path)}`;
  }
  // Een wet maken uit een geüpload document: target_path draagt de geüploade
  // bestandsnaam (COALESCE in list_running_task_jobs_for_account).
  if (job?.job_type === 'law_convert') {
    return `Wachten op wet maken van ${fileName(job.target_path, 'document')}`;
  }
  // Een wet ophalen in een traject: law_id draagt het BWB-id van de op te halen
  // wet, die nog niet in het corpus staat, dus lawName valt hier meestal terug.
  if (job?.job_type === 'traject_harvest') {
    return `Wachten op wet ophalen van ${lawName(job.law_id)}`;
  }
  return `Wachten op verrijking van ${lawName(job?.law_id)}`;
}
