// Verhuisd naar @regelrecht/frontend-shared. Deze shim zet de casusnaam die de
// sleutelprefix bepaalt en houdt de imports van deze app werkend.
//
// De casus staat hier en niet in het gedeelde bestand omdat de twee OCW-pocs
// hosted achter hetzelfde portaal op dezelfde origin draaien; zonder eigen
// prefix zouden ze elkaars keuzes lezen.
import { stelCasusIn } from '@regelrecht/frontend-shared/useBewaardeStand.js';

stelCasusIn('nieuwkomersbekostiging');

export {
  bewaardeRef,
  leesStand,
  bewaarStand,
  beschermSleutel,
  wisStand,
  heeftBewaardeStand,
} from '@regelrecht/frontend-shared/useBewaardeStand.js';
