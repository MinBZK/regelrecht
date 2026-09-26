// Design-system components this app renders, one entry point each.
// The package root would pull in all ~110 components; this list is generated
// from the nldd-* tags in the source and checked on every build by
// script/check-nldd-imports.mjs, so a newly used component fails the build
// instead of silently never upgrading.
//
// A component this app only names in prose (markdown inline code) is absent
// here on purpose: such a name has to exist, not to be imported.
//
// Regenerate: npm run nldd:imports
import '@nldd/design-system/button';
import '@nldd/design-system/button-group';
import '@nldd/design-system/cell';
import '@nldd/design-system/checkbox-field';
import '@nldd/design-system/code-viewer';
import '@nldd/design-system/container';
import '@nldd/design-system/date-field';
import '@nldd/design-system/dropdown';
import '@nldd/design-system/form';
import '@nldd/design-system/form-actions';
import '@nldd/design-system/form-field';
import '@nldd/design-system/form-section';
import '@nldd/design-system/icon-button';
import '@nldd/design-system/inline-dialog';
import '@nldd/design-system/number-field';
import '@nldd/design-system/page';
import '@nldd/design-system/rich-text';
import '@nldd/design-system/segmented-control';
import '@nldd/design-system/simple-section';
import '@nldd/design-system/spacer';
import '@nldd/design-system/tab-bar';
import '@nldd/design-system/table';
import '@nldd/design-system/text-cell';
import '@nldd/design-system/text-field';
import '@nldd/design-system/title';
import '@nldd/design-system/top-navigation-bar';
