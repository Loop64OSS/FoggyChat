import type { Snippet } from 'svelte';
export interface AppBarProps {
    /** Set base styles. */
    base?: string;
    /** Set background styles. */
    background?: string;
    /** Set vertical spacing styles. */
    spaceY?: string;
    /** Set border styles. */
    border?: string;
    /** Set padding styles. */
    padding?: string;
    /** Set shadow styles. */
    shadow?: string;
    /** Provide arbitrary CSS classes. */
    classes?: string;
    /** Sets toolbar's base styles. */
    toolbarBase?: string;
    /** Sets toolbar's grid columns styles. */
    toolbarGridCols?: string;
    /** Sets toolbar's gap styles. */
    toolbarGap?: string;
    /** Provide arbitrary CSS classes to the toolbar. */
    toolbarClasses?: string;
    /** Sets the lead snippet element's base styles. */
    leadBase?: string;
    /** Sets the lead snippet element's horizontal spacing styles. */
    leadSpaceX?: string;
    /** Sets the lead snippet element's padding styles. */
    leadPadding?: string;
    /** Provide arbitrary CSS classes to the lead snippet. */
    leadClasses?: string;
    /** Sets the center snippet element's base styles. */
    centerBase?: string;
    /** Sets the center snippet element's alignment styles. */
    centerAlign?: string;
    /** Sets the center snippet element's padding styles. */
    centerPadding?: string;
    /** Provide arbitrary CSS classes to the center snippet. */
    centerClasses?: string;
    /** Sets the trail snippet element's base styles. */
    trailBase?: string;
    /** Sets the trail snippet element's horizontal spacing styles. */
    trailSpaceX?: string;
    /** Sets the trail snippet element's padding styles. */
    trailPadding?: string;
    /** Provide arbitrary CSS classes to the trail snippet. */
    trailClasses?: string;
    /** Sets the headline snippet element's base styles. */
    headlineBase?: string;
    /** Provide arbitrary CSS classes to the headline snippet. */
    headlineClasses?: string;
    /** The center slot. */
    children?: Snippet;
    /** The lead slot. */
    lead?: Snippet;
    /** The trail slot. */
    trail?: Snippet;
    /** The headline slot. */
    headline?: Snippet;
}
