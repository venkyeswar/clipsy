/**
 * The page width shared by the header, sections and footer. The width is fluid up to 90rem
 * (120rem on very wide screens), and the side padding grows with the viewport. Zoomed-out
 * browsers get a wider layout instead of a narrow centred column.
 */
export const PAGE_CONTAINER = "mx-auto w-full max-w-[90rem] 3xl:max-w-[120rem] px-[clamp(1rem,3vw,3rem)]";

/** Vertical rhythm between page sections. */
export const SECTION_PADDING = "py-[clamp(2rem,4vw,4.5rem)]";
