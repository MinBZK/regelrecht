// Regelrecht Landing Page JavaScript

// Demo functionality
function executeRule() {
    const birthdateInput = document.getElementById('birthdate');
    const ruleSelect = document.getElementById('rule');
    const resultDisplay = document.getElementById('result');
    
    const birthdate = new Date(birthdateInput.value);
    const rule = ruleSelect.value;
    
    // Show loading state
    resultDisplay.innerHTML = '<p style="color: #767676;">Bezig met uitvoeren...</p>';
    
    // Simulate processing time
    setTimeout(() => {
        let result = '';
        
        switch(rule) {
            case 'pensioenleeftijd':
                result = calculatePensionAge(birthdate);
                break;
            case 'kinderbijslag':
                result = calculateChildBenefit(birthdate);
                break;
            default:
                result = 'Onbekende regel geselecteerd.';
        }
        
        resultDisplay.innerHTML = result;
    }, 1000);
}

function calculatePensionAge(birthdate) {
    const birthYear = birthdate.getFullYear();
    let pensionAge;
    let reasoning = '';
    
    // Simplified Dutch pension age rules
    if (birthYear < 1955) {
        pensionAge = 65;
        reasoning = `
            <div style="background: #f7f7f7; padding: 1rem; border-radius: 0.5rem; margin-bottom: 1rem;">
                <h4 style="color: #01689b; margin-bottom: 0.5rem;">Regeluitvoering:</h4>
                <p><strong>Voorwaarde:</strong> Geboortejaar (${birthYear}) < 1955</p>
                <p><strong>Resultaat:</strong> Pensioenleeftijd = 65 jaar</p>
                <p><strong>Juridische basis:</strong> AOW-wet artikel 7a</p>
            </div>
        `;
    } else if (birthYear >= 1955 && birthYear < 1960) {
        // Gradual increase from 65 to 67
        const monthsToAdd = Math.floor((birthYear - 1955) * 12 / 5);
        const years = Math.floor(monthsToAdd / 12);
        const months = monthsToAdd % 12;
        pensionAge = 65 + years + (months > 0 ? ` jaar en ${months} maanden` : ' jaar');
        reasoning = `
            <div style="background: #f7f7f7; padding: 1rem; border-radius: 0.5rem; margin-bottom: 1rem;">
                <h4 style="color: #01689b; margin-bottom: 0.5rem;">Regeluitvoering:</h4>
                <p><strong>Voorwaarde:</strong> 1955 ≤ Geboortejaar (${birthYear}) < 1960</p>
                <p><strong>Berekening:</strong> Geleidelijke verhoging van 65 naar 67 jaar</p>
                <p><strong>Resultaat:</strong> Pensioenleeftijd = ${pensionAge}</p>
                <p><strong>Juridische basis:</strong> AOW-wet artikel 7a, overgangsregeling</p>
            </div>
        `;
    } else {
        pensionAge = 67;
        reasoning = `
            <div style="background: #f7f7f7; padding: 1rem; border-radius: 0.5rem; margin-bottom: 1rem;">
                <h4 style="color: #01689b; margin-bottom: 0.5rem;">Regeluitvoering:</h4>
                <p><strong>Voorwaarde:</strong> Geboortejaar (${birthYear}) ≥ 1960</p>
                <p><strong>Resultaat:</strong> Pensioenleeftijd = 67 jaar</p>
                <p><strong>Juridische basis:</strong> AOW-wet artikel 7a</p>
            </div>
        `;
    }
    
    return `
        ${reasoning}
        <div style="background: #01689b; color: white; padding: 1rem; border-radius: 0.5rem;">
            <h4 style="margin-bottom: 0.5rem;">Uitkomst</h4>
            <p style="font-size: 1.2rem; font-weight: 600;">Pensioenleeftijd: ${pensionAge}</p>
        </div>
        <div style="margin-top: 1rem; padding: 1rem; background: #e8f4fd; border-radius: 0.5rem; border-left: 4px solid #01689b;">
            <h5 style="color: #01689b; margin-bottom: 0.5rem;">Wat betekent dit?</h5>
            <p style="color: #154273; font-size: 0.9rem;">
                Deze berekening is gebaseerd op de huidige AOW-wetgeving. De uitkomst is 
                volledig transparant en controleerbaar doordat de regels expliciet in code 
                zijn vastgelegd in plaats van verborgen in complexe systemen.
            </p>
        </div>
    `;
}

function calculateChildBenefit(birthdate) {
    const today = new Date();
    const age = Math.floor((today - birthdate) / (365.25 * 24 * 60 * 60 * 1000));
    
    let benefit = 0;
    let eligible = false;
    let reasoning = '';
    
    if (age < 18) {
        eligible = true;
        if (age <= 5) {
            benefit = 250.89; // Per kwartaal voor 0-5 jaar
        } else if (age <= 11) {
            benefit = 304.48; // Per kwartaal voor 6-11 jaar
        } else {
            benefit = 358.07; // Per kwartaal voor 12-17 jaar
        }
        
        reasoning = `
            <div style="background: #f7f7f7; padding: 1rem; border-radius: 0.5rem; margin-bottom: 1rem;">
                <h4 style="color: #01689b; margin-bottom: 0.5rem;">Regeluitvoering:</h4>
                <p><strong>Voorwaarde:</strong> Leeftijd (${age} jaar) < 18 jaar</p>
                <p><strong>Leeftijdscategorie:</strong> ${age <= 5 ? '0-5 jaar' : age <= 11 ? '6-11 jaar' : '12-17 jaar'}</p>
                <p><strong>Resultaat:</strong> Wel recht op kinderbijslag</p>
                <p><strong>Juridische basis:</strong> Algemene Kinderbijslagwet artikel 7</p>
            </div>
        `;
    } else {
        reasoning = `
            <div style="background: #f7f7f7; padding: 1rem; border-radius: 0.5rem; margin-bottom: 1rem;">
                <h4 style="color: #01689b; margin-bottom: 0.5rem;">Regeluitvoering:</h4>
                <p><strong>Voorwaarde:</strong> Leeftijd (${age} jaar) ≥ 18 jaar</p>
                <p><strong>Resultaat:</strong> Geen recht op kinderbijslag</p>
                <p><strong>Juridische basis:</strong> Algemene Kinderbijslagwet artikel 7</p>
            </div>
        `;
    }
    
    const resultContent = eligible ? 
        `<p style="font-size: 1.2rem; font-weight: 600;">€${benefit} per kwartaal</p>` :
        `<p style="font-size: 1.2rem; font-weight: 600;">Geen recht op kinderbijslag</p>`;
    
    return `
        ${reasoning}
        <div style="background: ${eligible ? '#01689b' : '#d52b1e'}; color: white; padding: 1rem; border-radius: 0.5rem;">
            <h4 style="margin-bottom: 0.5rem;">Uitkomst</h4>
            ${resultContent}
        </div>
        <div style="margin-top: 1rem; padding: 1rem; background: #e8f4fd; border-radius: 0.5rem; border-left: 4px solid #01689b;">
            <h5 style="color: #01689b; margin-bottom: 0.5rem;">Transparantie in actie</h5>
            <p style="color: #154273; font-size: 0.9rem;">
                ${eligible ? 
                    'Deze berekening toont precies welke regels zijn toegepast en waarom u recht heeft op kinderbijslag.' :
                    'De regels zijn duidelijk: kinderbijslag geldt alleen tot 18 jaar. Geen verborgen voorwaarden of onduidelijkheden.'
                }
                Dit is het verschil dat Regelrecht maakt: volledige transparantie over hoe overheidsbeslissingen tot stand komen.
            </p>
        </div>
    `;
}

// Smooth scrolling for anchor links
document.addEventListener('DOMContentLoaded', function() {
    // Smooth scrolling for navigation links
    const navLinks = document.querySelectorAll('a[href^="#"]');
    
    navLinks.forEach(link => {
        link.addEventListener('click', function(e) {
            e.preventDefault();
            
            const targetId = this.getAttribute('href').substring(1);
            const targetElement = document.getElementById(targetId);
            
            if (targetElement) {
                const offsetTop = targetElement.offsetTop - 80; // Account for fixed nav
                
                window.scrollTo({
                    top: offsetTop,
                    behavior: 'smooth'
                });
            }
        });
    });
    
    // Add animation to feature cards on scroll
    const observerOptions = {
        threshold: 0.1,
        rootMargin: '0px 0px -100px 0px'
    };
    
    const observer = new IntersectionObserver(function(entries) {
        entries.forEach(entry => {
            if (entry.isIntersecting) {
                entry.target.style.opacity = '1';
                entry.target.style.transform = 'translateY(0)';
            }
        });
    }, observerOptions);
    
    // Observe feature cards
    const featureCards = document.querySelectorAll('.feature-card, .solution-card');
    featureCards.forEach(card => {
        card.style.opacity = '0';
        card.style.transform = 'translateY(20px)';
        card.style.transition = 'opacity 0.6s ease, transform 0.6s ease';
        observer.observe(card);
    });
    
    // FAQ accordion behavior is handled by HTML details/summary
});


// Add loading animation for demo
function showLoading(element) {
    let dots = '';
    const loadingInterval = setInterval(() => {
        dots = dots.length >= 3 ? '' : dots + '.';
        element.innerHTML = `<p style="color: #767676;">Bezig met uitvoeren${dots}</p>`;
    }, 500);
    
    return loadingInterval;
}

// Add error handling for demo
function handleDemoError(error) {
    const resultDisplay = document.getElementById('result');
    resultDisplay.innerHTML = `
        <div style="background: #d52b1e; color: white; padding: 1rem; border-radius: 0.5rem;">
            <h4 style="margin-bottom: 0.5rem;">Fout opgetreden</h4>
            <p>Er is een fout opgetreden bij het uitvoeren van de regel. Probeer het opnieuw.</p>
        </div>
    `;
    console.error('Demo error:', error);
}

// Newsletter signup functionality
document.addEventListener('DOMContentLoaded', function() {
    const newsletterForm = document.querySelector('.newsletter-form');
    if (newsletterForm) {
        newsletterForm.addEventListener('submit', function(e) {
            e.preventDefault();
            
            const emailInput = this.querySelector('.newsletter-input');
            const email = emailInput.value.trim();
            
            if (!email) {
                alert('Vul een geldig e-mailadres in.');
                return;
            }
            
            // Simulate newsletter signup
            const button = this.querySelector('.newsletter-btn');
            const originalText = button.textContent;
            
            button.textContent = 'Aanmelden...';
            button.disabled = true;
            
            setTimeout(() => {
                button.textContent = '✓ Aangemeld!';
                button.style.background = 'var(--rvo-green)';
                emailInput.value = '';
                
                setTimeout(() => {
                    button.textContent = originalText;
                    button.disabled = false;
                    button.style.background = '';
                }, 3000);
            }, 1500);
        });
    }
});

// Export functions for global access
window.executeRule = executeRule;