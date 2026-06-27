// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="index.html">Introduction</a></span></li><li class="chapter-item expanded "><li class="part-title">Conformity and evidence</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="conformity/index.html"><strong aria-hidden="true">1.</strong> Conformity first</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="conformity/claim-boundary.html"><strong aria-hidden="true">1.1.</strong> Claim boundary</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="conformity/evidence-model.html"><strong aria-hidden="true">1.2.</strong> Evidence model</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="conformity/what-is-tested.html"><strong aria-hidden="true">1.3.</strong> What is tested</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="conformity/what-is-not-certified.html"><strong aria-hidden="true">1.4.</strong> What is not certified</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="conformity/hardware-and-aef.html"><strong aria-hidden="true">1.5.</strong> Hardware and AEF path</a></span></li></ol><li class="chapter-item expanded "><li class="part-title">Understand ISOBUS</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="isobus-basics/index.html"><strong aria-hidden="true">2.</strong> ISOBUS in plain words</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="isobus-basics/reading-candump-traces.html"><strong aria-hidden="true">2.1.</strong> Reading candump traces</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="isobus-basics/further-reading.html"><strong aria-hidden="true">2.2.</strong> Further reading</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/index.html"><strong aria-hidden="true">3.</strong> The standards, end to end</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/standards-capability-map.html"><strong aria-hidden="true">3.1.</strong> Standards capability map</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/foundations.html"><strong aria-hidden="true">3.2.</strong> The networking foundation</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-general-device-classes.html"><strong aria-hidden="true">3.2.1.</strong> ISO 11783-1: general &amp; device classes</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-physical-layer.html"><strong aria-hidden="true">3.2.2.</strong> ISO 11783-2: the physical layer</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/j1939.html"><strong aria-hidden="true">3.2.3.</strong> SAE J1939: the heritage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-datalink-transport.html"><strong aria-hidden="true">3.2.4.</strong> ISO 11783-3: data link &amp; transport</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-network-layer.html"><strong aria-hidden="true">3.2.5.</strong> ISO 11783-4: the network layer</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-network-management.html"><strong aria-hidden="true">3.2.6.</strong> ISO 11783-5: network management &amp; address claiming</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/virtual-terminal.html"><strong aria-hidden="true">3.3.</strong> The Virtual Terminal (ISO 11783-6)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/task-controller.html"><strong aria-hidden="true">3.4.</strong> The Task Controller (ISO 11783-10)</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-data-dictionary.html"><strong aria-hidden="true">3.4.1.</strong> ISO 11783-11: the data dictionary</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/implement-and-services.html"><strong aria-hidden="true">3.5.</strong> Application services</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-implement-messages.html"><strong aria-hidden="true">3.5.1.</strong> ISO 11783-7: implement messages</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-tractor-ecu.html"><strong aria-hidden="true">3.5.2.</strong> ISO 11783-9: the tractor ECU</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-diagnostics.html"><strong aria-hidden="true">3.5.3.</strong> ISO 11783-12: diagnostics</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-file-server.html"><strong aria-hidden="true">3.5.4.</strong> ISO 11783-13: the File Server</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/iso11783-sequence-control.html"><strong aria-hidden="true">3.5.5.</strong> ISO 11783-14: sequence control</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/tim.html"><strong aria-hidden="true">3.5.6.</strong> TIM (AEF)</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/autosteer.html"><strong aria-hidden="true">3.5.7.</strong> Automatic guidance (autosteer)</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="standards/positioning.html"><strong aria-hidden="true">3.6.</strong> Positioning: NMEA and GNSS</a></span></li></ol><li class="chapter-item expanded "><li class="part-title">Reading the bus (CAN · J1939 · NMEA 2000)</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="decode/index.html"><strong aria-hidden="true">4.</strong> Decode without ISOBUS</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="decode/can-frames.html"><strong aria-hidden="true">4.1.</strong> Anatomy of a CAN frame</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="decode/j1939-messages.html"><strong aria-hidden="true">4.2.</strong> J1939 messages and PGNs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="decode/nmea2000.html"><strong aria-hidden="true">4.3.</strong> NMEA 2000 on the bus</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="decode/tut-inspect-can.html"><strong aria-hidden="true">5.</strong> Tutorial: inspect CAN identifiers</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="decode/tut-j1939.html"><strong aria-hidden="true">6.</strong> Tutorial: decode J1939 PGNs</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="decode/tut-nmea2000.html"><strong aria-hidden="true">7.</strong> Tutorial: decode NMEA 2000</a></span></li><li class="chapter-item expanded "><li class="part-title">Build with machbus</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="getting-started/index.html"><strong aria-hidden="true">8.</strong> Getting started</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="getting-started/install-rust.html"><strong aria-hidden="true">8.1.</strong> Install Rust</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="getting-started/build-and-verify.html"><strong aria-hidden="true">8.2.</strong> Build and verify</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="getting-started/no-std-microcontrollers.html"><strong aria-hidden="true">8.3.</strong> no_std on microcontrollers</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="getting-started/first-node.html"><strong aria-hidden="true">8.4.</strong> First node</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="getting-started/virtual-bus.html"><strong aria-hidden="true">8.5.</strong> Virtual bus</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="getting-started/socketcan.html"><strong aria-hidden="true">8.6.</strong> SocketCAN</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="getting-started/logging-and-traces.html"><strong aria-hidden="true">8.7.</strong> Logging and traces</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/session-facade.html"><strong aria-hidden="true">9.</strong> The session facade</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/index.html"><strong aria-hidden="true">10.</strong> Guided walkthrough</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/hello-world.html"><strong aria-hidden="true">10.1.</strong> 1. The ISOBUS Hello World</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/hello-world-explained.html"><strong aria-hidden="true">10.2.</strong> 2. The Hello World, line by line</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/sending-receiving.html"><strong aria-hidden="true">10.3.</strong> 3. Sending and receiving messages</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/requests-and-acks.html"><strong aria-hidden="true">10.4.</strong> 4. Requests and acknowledgements</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/transport.html"><strong aria-hidden="true">10.5.</strong> 5. Moving big data with transport</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/diagnostics.html"><strong aria-hidden="true">10.6.</strong> 6. Talking diagnostics</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/virtual-terminal.html"><strong aria-hidden="true">10.7.</strong> 7. Your first Virtual Terminal client</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/task-controller.html"><strong aria-hidden="true">10.8.</strong> 8. Your first Task Controller client</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/tractor-and-implement.html"><strong aria-hidden="true">10.9.</strong> 9. Tractor and implement personas</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/real-hardware.html"><strong aria-hidden="true">10.10.</strong> 10. Onto real hardware with SocketCAN</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/async-events.html"><strong aria-hidden="true">10.11.</strong> 11. Async event streams</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="guide/capstone.html"><strong aria-hidden="true">10.12.</strong> 12. Capstone: a complete implement ECU</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/index.html"><strong aria-hidden="true">11.</strong> Tutorials</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><span><strong aria-hidden="true">11.1.</strong> Network basics</span></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/address-claim.html"><strong aria-hidden="true">11.1.1.</strong> Address claim</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/request-pgn.html"><strong aria-hidden="true">11.1.2.</strong> PGN request</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/name-management.html"><strong aria-hidden="true">11.1.3.</strong> NAME management</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/transport-protocol.html"><strong aria-hidden="true">11.1.4.</strong> Transport Protocol</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/fast-packet.html"><strong aria-hidden="true">11.1.5.</strong> Fast Packet</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/network-routing.html"><strong aria-hidden="true">11.1.6.</strong> Network routing</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><span><strong aria-hidden="true">11.2.</strong> Virtual Terminal</span></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/virtual-terminal-client.html"><strong aria-hidden="true">11.2.1.</strong> Virtual Terminal client</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/virtual-terminal-server.html"><strong aria-hidden="true">11.2.2.</strong> Virtual Terminal server</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/vt-object-pools.html"><strong aria-hidden="true">11.2.3.</strong> VT object pools</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/vt-updates.html"><strong aria-hidden="true">11.2.4.</strong> VT updates</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/vt-auxiliary-capabilities.html"><strong aria-hidden="true">11.2.5.</strong> VT auxiliary capabilities</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><span><strong aria-hidden="true">11.3.</strong> Task Controller</span></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/task-controller-client.html"><strong aria-hidden="true">11.3.1.</strong> Task Controller client</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/task-controller-server.html"><strong aria-hidden="true">11.3.2.</strong> Task Controller server</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/ddop.html"><strong aria-hidden="true">11.3.3.</strong> DDOP</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/tc-geo-prescription.html"><strong aria-hidden="true">11.3.4.</strong> TC-GEO prescription</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><span><strong aria-hidden="true">11.4.</strong> Implement and tractor</span></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/tractor-ecu.html"><strong aria-hidden="true">11.4.1.</strong> Tractor ECU</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/implement-ecu.html"><strong aria-hidden="true">11.4.2.</strong> Implement ECU</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/powertrain.html"><strong aria-hidden="true">11.4.3.</strong> Powertrain</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/guidance.html"><strong aria-hidden="true">11.4.4.</strong> Guidance</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/sequence-control.html"><strong aria-hidden="true">11.4.5.</strong> Sequence Control</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/tim.html"><strong aria-hidden="true">11.4.6.</strong> TIM and automation</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><span><strong aria-hidden="true">11.5.</strong> Diagnostics and files</span></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/diagnostics.html"><strong aria-hidden="true">11.5.1.</strong> Diagnostics</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/file-server.html"><strong aria-hidden="true">11.5.2.</strong> File Server</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><span><strong aria-hidden="true">11.6.</strong> Positioning</span></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/nmea-2000.html"><strong aria-hidden="true">11.6.1.</strong> NMEA 2000</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/serial-gnss.html"><strong aria-hidden="true">11.6.2.</strong> Serial GNSS</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><span><strong aria-hidden="true">11.7.</strong> Tooling and validation</span></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/socketcan-replay.html"><strong aria-hidden="true">11.7.1.</strong> SocketCAN replay</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="tutorials/fuzz-and-validation.html"><strong aria-hidden="true">11.7.2.</strong> Fuzz and validation</a></span></li></ol></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="examples/index.html"><strong aria-hidden="true">12.</strong> Examples</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="examples/minimal.html"><strong aria-hidden="true">12.1.</strong> Minimal</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="examples/tractor.html"><strong aria-hidden="true">12.2.</strong> Tractor</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="examples/implement.html"><strong aria-hidden="true">12.3.</strong> Implement</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="examples/vt-server.html"><strong aria-hidden="true">12.4.</strong> VT server</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="examples/tc-server.html"><strong aria-hidden="true">12.5.</strong> TC server</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="examples/file-server.html"><strong aria-hidden="true">12.6.</strong> File Server</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="examples/full-scenario.html"><strong aria-hidden="true">12.7.</strong> Full scenario</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="bindings/index.html"><strong aria-hidden="true">13.</strong> Bindings</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="bindings/c.html"><strong aria-hidden="true">13.1.</strong> C ABI</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="bindings/python.html"><strong aria-hidden="true">13.2.</strong> Python</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="bindings/abi-stability.html"><strong aria-hidden="true">13.3.</strong> ABI stability</a></span></li></ol><li class="chapter-item expanded "><li class="part-title">Reference</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/index.html"><strong aria-hidden="true">14.</strong> Reference overview</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/crate-map.html"><strong aria-hidden="true">14.1.</strong> Crate map</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/role-boundaries.html"><strong aria-hidden="true">14.2.</strong> Role boundaries</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/feature-flags.html"><strong aria-hidden="true">14.3.</strong> Feature flags</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/error-handling.html"><strong aria-hidden="true">14.4.</strong> Error handling</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/validation-gates.html"><strong aria-hidden="true">14.5.</strong> Validation gates</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/protocol-matrix.html"><strong aria-hidden="true">14.6.</strong> Protocol matrix</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/protocol-coverage.html"><strong aria-hidden="true">14.7.</strong> Protocol coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/vt-render-coverage.html"><strong aria-hidden="true">14.8.</strong> VT render coverage</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/standard-gap-roadmap.html"><strong aria-hidden="true">14.9.</strong> Standard gap roadmap</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/hardware-evidence.html"><strong aria-hidden="true">14.10.</strong> Hardware evidence</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/release.html"><strong aria-hidden="true">14.11.</strong> Release checklist</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/validation-history.html"><strong aria-hidden="true">14.12.</strong> Validation history</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/behavior-differences.html"><strong aria-hidden="true">14.13.</strong> Behavior differences</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/glossary.html"><strong aria-hidden="true">14.14.</strong> Glossary</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><span><strong aria-hidden="true">14.15.</strong> Audit and hardening history</span></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/audit/bindings.html"><strong aria-hidden="true">14.15.1.</strong> Audit binding contracts</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/audit/conformance.html"><strong aria-hidden="true">14.15.2.</strong> Audit conformance boundary</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/audit/hardening-plan.html"><strong aria-hidden="true">14.15.3.</strong> Audit hardening plan</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="reference/audit/original-hardening-plan.html"><strong aria-hidden="true">14.15.4.</strong> Original hardening plan</a></span></li></ol></li></ol><li class="chapter-item expanded "><li class="part-title">Troubleshooting</li></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="troubleshooting/index.html"><strong aria-hidden="true">15.</strong> Troubleshooting overview</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="troubleshooting/build.html"><strong aria-hidden="true">15.1.</strong> Build problems</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="troubleshooting/can-interface.html"><strong aria-hidden="true">15.2.</strong> CAN interface problems</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="troubleshooting/address-conflicts.html"><strong aria-hidden="true">15.3.</strong> Address conflicts</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="troubleshooting/vt-upload.html"><strong aria-hidden="true">15.4.</strong> VT upload problems</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="troubleshooting/tc-ddop.html"><strong aria-hidden="true">15.5.</strong> TC/DDOP problems</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="troubleshooting/bindings.html"><strong aria-hidden="true">15.6.</strong> Binding problems</a></span></li></ol></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split('#')[0].split('?')[0];
        if (current_page.endsWith('/')) {
            current_page += 'index.html';
        }
        const links = Array.prototype.slice.call(this.querySelectorAll('a'));
        const l = links.length;
        for (let i = 0; i < l; ++i) {
            const link = links[i];
            const href = link.getAttribute('href');
            if (href && !href.startsWith('#') && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The 'index' page is supposed to alias the first chapter in the book.
            // Check both with and without the '.html' suffix to be robust against pretty URLs
            if (link.href.replace(/\.html$/, '') === current_page.replace(/\.html$/, '')
                || i === 0
                && path_to_root === ''
                && current_page.endsWith('/index.html')) {
                link.classList.add('active');
                let parent = link.parentElement;
                while (parent) {
                    if (parent.tagName === 'LI' && parent.classList.contains('chapter-item')) {
                        parent.classList.add('expanded');
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', e => {
            if (e.target.tagName === 'A') {
                const clientRect = e.target.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                sessionStorage.setItem('sidebar-scroll-offset', clientRect.top - sidebarRect.top);
            }
        }, { passive: true });
        const sidebarScrollOffset = sessionStorage.getItem('sidebar-scroll-offset');
        sessionStorage.removeItem('sidebar-scroll-offset');
        if (sidebarScrollOffset !== null) {
            // preserve sidebar scroll position when navigating via links within sidebar
            const activeSection = this.querySelector('.active');
            if (activeSection) {
                const clientRect = activeSection.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                const currentOffset = clientRect.top - sidebarRect.top;
                this.scrollTop += currentOffset - parseFloat(sidebarScrollOffset);
            }
        } else {
            // scroll sidebar to current active section when navigating via
            // 'next/previous chapter' buttons
            const activeSection = document.querySelector('#mdbook-sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        const sidebarAnchorToggles = document.querySelectorAll('.chapter-fold-toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(el => {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define('mdbook-sidebar-scrollbox', MDBookSidebarScrollbox);


// ---------------------------------------------------------------------------
// Support for dynamically adding headers to the sidebar.

(function() {
    // This is used to detect which direction the page has scrolled since the
    // last scroll event.
    let lastKnownScrollPosition = 0;
    // This is the threshold in px from the top of the screen where it will
    // consider a header the "current" header when scrolling down.
    const defaultDownThreshold = 150;
    // Same as defaultDownThreshold, except when scrolling up.
    const defaultUpThreshold = 300;
    // The threshold is a virtual horizontal line on the screen where it
    // considers the "current" header to be above the line. The threshold is
    // modified dynamically to handle headers that are near the bottom of the
    // screen, and to slightly offset the behavior when scrolling up vs down.
    let threshold = defaultDownThreshold;
    // This is used to disable updates while scrolling. This is needed when
    // clicking the header in the sidebar, which triggers a scroll event. It
    // is somewhat finicky to detect when the scroll has finished, so this
    // uses a relatively dumb system of disabling scroll updates for a short
    // time after the click.
    let disableScroll = false;
    // Array of header elements on the page.
    let headers;
    // Array of li elements that are initially collapsed headers in the sidebar.
    // I'm not sure why eslint seems to have a false positive here.
    // eslint-disable-next-line prefer-const
    let headerToggles = [];
    // This is a debugging tool for the threshold which you can enable in the console.
    let thresholdDebug = false;

    // Updates the threshold based on the scroll position.
    function updateThreshold() {
        const scrollTop = window.pageYOffset || document.documentElement.scrollTop;
        const windowHeight = window.innerHeight;
        const documentHeight = document.documentElement.scrollHeight;

        // The number of pixels below the viewport, at most documentHeight.
        // This is used to push the threshold down to the bottom of the page
        // as the user scrolls towards the bottom.
        const pixelsBelow = Math.max(0, documentHeight - (scrollTop + windowHeight));
        // The number of pixels above the viewport, at least defaultDownThreshold.
        // Similar to pixelsBelow, this is used to push the threshold back towards
        // the top when reaching the top of the page.
        const pixelsAbove = Math.max(0, defaultDownThreshold - scrollTop);
        // How much the threshold should be offset once it gets close to the
        // bottom of the page.
        const bottomAdd = Math.max(0, windowHeight - pixelsBelow - defaultDownThreshold);
        let adjustedBottomAdd = bottomAdd;

        // Adjusts bottomAdd for a small document. The calculation above
        // assumes the document is at least twice the windowheight in size. If
        // it is less than that, then bottomAdd needs to be shrunk
        // proportional to the difference in size.
        if (documentHeight < windowHeight * 2) {
            const maxPixelsBelow = documentHeight - windowHeight;
            const t = 1 - pixelsBelow / Math.max(1, maxPixelsBelow);
            const clamp = Math.max(0, Math.min(1, t));
            adjustedBottomAdd *= clamp;
        }

        let scrollingDown = true;
        if (scrollTop < lastKnownScrollPosition) {
            scrollingDown = false;
        }

        if (scrollingDown) {
            // When scrolling down, move the threshold up towards the default
            // downwards threshold position. If near the bottom of the page,
            // adjustedBottomAdd will offset the threshold towards the bottom
            // of the page.
            const amountScrolledDown = scrollTop - lastKnownScrollPosition;
            const adjustedDefault = defaultDownThreshold + adjustedBottomAdd;
            threshold = Math.max(adjustedDefault, threshold - amountScrolledDown);
        } else {
            // When scrolling up, move the threshold down towards the default
            // upwards threshold position. If near the bottom of the page,
            // quickly transition the threshold back up where it normally
            // belongs.
            const amountScrolledUp = lastKnownScrollPosition - scrollTop;
            const adjustedDefault = defaultUpThreshold - pixelsAbove
                + Math.max(0, adjustedBottomAdd - defaultDownThreshold);
            threshold = Math.min(adjustedDefault, threshold + amountScrolledUp);
        }

        if (documentHeight <= windowHeight) {
            threshold = 0;
        }

        if (thresholdDebug) {
            const id = 'mdbook-threshold-debug-data';
            let data = document.getElementById(id);
            if (data === null) {
                data = document.createElement('div');
                data.id = id;
                data.style.cssText = `
                    position: fixed;
                    top: 50px;
                    right: 10px;
                    background-color: 0xeeeeee;
                    z-index: 9999;
                    pointer-events: none;
                `;
                document.body.appendChild(data);
            }
            data.innerHTML = `
                <table>
                  <tr><td>documentHeight</td><td>${documentHeight.toFixed(1)}</td></tr>
                  <tr><td>windowHeight</td><td>${windowHeight.toFixed(1)}</td></tr>
                  <tr><td>scrollTop</td><td>${scrollTop.toFixed(1)}</td></tr>
                  <tr><td>pixelsAbove</td><td>${pixelsAbove.toFixed(1)}</td></tr>
                  <tr><td>pixelsBelow</td><td>${pixelsBelow.toFixed(1)}</td></tr>
                  <tr><td>bottomAdd</td><td>${bottomAdd.toFixed(1)}</td></tr>
                  <tr><td>adjustedBottomAdd</td><td>${adjustedBottomAdd.toFixed(1)}</td></tr>
                  <tr><td>scrollingDown</td><td>${scrollingDown}</td></tr>
                  <tr><td>threshold</td><td>${threshold.toFixed(1)}</td></tr>
                </table>
            `;
            drawDebugLine();
        }

        lastKnownScrollPosition = scrollTop;
    }

    function drawDebugLine() {
        if (!document.body) {
            return;
        }
        const id = 'mdbook-threshold-debug-line';
        const existingLine = document.getElementById(id);
        if (existingLine) {
            existingLine.remove();
        }
        const line = document.createElement('div');
        line.id = id;
        line.style.cssText = `
            position: fixed;
            top: ${threshold}px;
            left: 0;
            width: 100vw;
            height: 2px;
            background-color: red;
            z-index: 9999;
            pointer-events: none;
        `;
        document.body.appendChild(line);
    }

    function mdbookEnableThresholdDebug() {
        thresholdDebug = true;
        updateThreshold();
        drawDebugLine();
    }

    window.mdbookEnableThresholdDebug = mdbookEnableThresholdDebug;

    // Updates which headers in the sidebar should be expanded. If the current
    // header is inside a collapsed group, then it, and all its parents should
    // be expanded.
    function updateHeaderExpanded(currentA) {
        // Add expanded to all header-item li ancestors.
        let current = currentA.parentElement;
        while (current) {
            if (current.tagName === 'LI' && current.classList.contains('header-item')) {
                current.classList.add('expanded');
            }
            current = current.parentElement;
        }
    }

    // Updates which header is marked as the "current" header in the sidebar.
    // This is done with a virtual Y threshold, where headers at or below
    // that line will be considered the current one.
    function updateCurrentHeader() {
        if (!headers || !headers.length) {
            return;
        }

        // Reset the classes, which will be rebuilt below.
        const els = document.getElementsByClassName('current-header');
        for (const el of els) {
            el.classList.remove('current-header');
        }
        for (const toggle of headerToggles) {
            toggle.classList.remove('expanded');
        }

        // Find the last header that is above the threshold.
        let lastHeader = null;
        for (const header of headers) {
            const rect = header.getBoundingClientRect();
            if (rect.top <= threshold) {
                lastHeader = header;
            } else {
                break;
            }
        }
        if (lastHeader === null) {
            lastHeader = headers[0];
            const rect = lastHeader.getBoundingClientRect();
            const windowHeight = window.innerHeight;
            if (rect.top >= windowHeight) {
                return;
            }
        }

        // Get the anchor in the summary.
        const href = '#' + lastHeader.id;
        const a = [...document.querySelectorAll('.header-in-summary')]
            .find(element => element.getAttribute('href') === href);
        if (!a) {
            return;
        }

        a.classList.add('current-header');

        updateHeaderExpanded(a);
    }

    // Updates which header is "current" based on the threshold line.
    function reloadCurrentHeader() {
        if (disableScroll) {
            return;
        }
        updateThreshold();
        updateCurrentHeader();
    }


    // When clicking on a header in the sidebar, this adjusts the threshold so
    // that it is located next to the header. This is so that header becomes
    // "current".
    function headerThresholdClick(event) {
        // See disableScroll description why this is done.
        disableScroll = true;
        setTimeout(() => {
            disableScroll = false;
        }, 100);
        // requestAnimationFrame is used to delay the update of the "current"
        // header until after the scroll is done, and the header is in the new
        // position.
        requestAnimationFrame(() => {
            requestAnimationFrame(() => {
                // Closest is needed because if it has child elements like <code>.
                const a = event.target.closest('a');
                const href = a.getAttribute('href');
                const targetId = href.substring(1);
                const targetElement = document.getElementById(targetId);
                if (targetElement) {
                    threshold = targetElement.getBoundingClientRect().bottom;
                    updateCurrentHeader();
                }
            });
        });
    }

    // Takes the nodes from the given head and copies them over to the
    // destination, along with some filtering.
    function filterHeader(source, dest) {
        const clone = source.cloneNode(true);
        clone.querySelectorAll('mark').forEach(mark => {
            mark.replaceWith(...mark.childNodes);
        });
        dest.append(...clone.childNodes);
    }

    // Scans page for headers and adds them to the sidebar.
    document.addEventListener('DOMContentLoaded', function() {
        const activeSection = document.querySelector('#mdbook-sidebar .active');
        if (activeSection === null) {
            return;
        }

        const main = document.getElementsByTagName('main')[0];
        headers = Array.from(main.querySelectorAll('h2, h3, h4, h5, h6'))
            .filter(h => h.id !== '' && h.children.length && h.children[0].tagName === 'A');

        if (headers.length === 0) {
            return;
        }

        // Build a tree of headers in the sidebar.

        const stack = [];

        const firstLevel = parseInt(headers[0].tagName.charAt(1));
        for (let i = 1; i < firstLevel; i++) {
            const ol = document.createElement('ol');
            ol.classList.add('section');
            if (stack.length > 0) {
                stack[stack.length - 1].ol.appendChild(ol);
            }
            stack.push({level: i + 1, ol: ol});
        }

        // The level where it will start folding deeply nested headers.
        const foldLevel = 3;

        for (let i = 0; i < headers.length; i++) {
            const header = headers[i];
            const level = parseInt(header.tagName.charAt(1));

            const currentLevel = stack[stack.length - 1].level;
            if (level > currentLevel) {
                // Begin nesting to this level.
                for (let nextLevel = currentLevel + 1; nextLevel <= level; nextLevel++) {
                    const ol = document.createElement('ol');
                    ol.classList.add('section');
                    const last = stack[stack.length - 1];
                    const lastChild = last.ol.lastChild;
                    // Handle the case where jumping more than one nesting
                    // level, which doesn't have a list item to place this new
                    // list inside of.
                    if (lastChild) {
                        lastChild.appendChild(ol);
                    } else {
                        last.ol.appendChild(ol);
                    }
                    stack.push({level: nextLevel, ol: ol});
                }
            } else if (level < currentLevel) {
                while (stack.length > 1 && stack[stack.length - 1].level > level) {
                    stack.pop();
                }
            }

            const li = document.createElement('li');
            li.classList.add('header-item');
            li.classList.add('expanded');
            if (level < foldLevel) {
                li.classList.add('expanded');
            }
            const span = document.createElement('span');
            span.classList.add('chapter-link-wrapper');
            const a = document.createElement('a');
            span.appendChild(a);
            a.href = '#' + header.id;
            a.classList.add('header-in-summary');
            filterHeader(header.children[0], a);
            a.addEventListener('click', headerThresholdClick);
            const nextHeader = headers[i + 1];
            if (nextHeader !== undefined) {
                const nextLevel = parseInt(nextHeader.tagName.charAt(1));
                if (nextLevel > level && level >= foldLevel) {
                    const toggle = document.createElement('a');
                    toggle.classList.add('chapter-fold-toggle');
                    toggle.classList.add('header-toggle');
                    toggle.addEventListener('click', () => {
                        li.classList.toggle('expanded');
                    });
                    const toggleDiv = document.createElement('div');
                    toggleDiv.textContent = '❱';
                    toggle.appendChild(toggleDiv);
                    span.appendChild(toggle);
                    headerToggles.push(li);
                }
            }
            li.appendChild(span);

            const currentParent = stack[stack.length - 1];
            currentParent.ol.appendChild(li);
        }

        const onThisPage = document.createElement('div');
        onThisPage.classList.add('on-this-page');
        onThisPage.append(stack[0].ol);
        const activeItemSpan = activeSection.parentElement;
        activeItemSpan.after(onThisPage);
    });

    document.addEventListener('DOMContentLoaded', reloadCurrentHeader);
    document.addEventListener('scroll', reloadCurrentHeader, { passive: true });
})();

