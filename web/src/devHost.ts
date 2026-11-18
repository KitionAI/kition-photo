/** The development host (dev.html): opens `public/sample/Sample.psd`, or a file chosen in the page. */
import { startDevHost } from '@kition/kit-dev-host'

// `host/pick` answers with workspace images (Place) and look files (Color Lookup's Load).
startDevHost({ sample: './sample/Sample.psd', pickable: /\.(png|jpe?g|gif|svg|webp|cube|3dl|look|csp)$/i })
