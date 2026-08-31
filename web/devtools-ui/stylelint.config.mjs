export default {
  extends: ['stylelint-config-standard-scss', 'stylelint-config-recommended-vue/scss'],
  plugins: ['stylelint-order'],
  rules: {
    'declaration-no-important': true,
    'max-nesting-depth': 3,
    'order/properties-alphabetical-order': true,
    'selector-class-pattern': '^[a-z][a-z0-9]*(?:(?:-|__)[a-z0-9]+)*(?:--[a-z0-9]+)?$',
    'selector-max-id': 0,
    'selector-max-specificity': '0,4,0',
  },
};
